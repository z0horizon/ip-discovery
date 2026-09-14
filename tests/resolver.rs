//! Unit tests for the resolver module — strategies, error paths, and helpers.

#[cfg(all(test, feature = "tokio"))]
mod resolver_tests {
    use ip_discovery::{
        Config, Error, IpVersion, Protocol, Provider, ProviderError, Resolver, Strategy,
    };
    use std::future::Future;
    use std::net::{IpAddr, Ipv4Addr};
    use std::pin::Pin;
    use std::time::Duration;

    // ── Mock providers ──────────────────────────────────────────────

    /// A provider that always returns the configured IP after an optional delay.
    struct MockProvider {
        name: String,
        ip: IpAddr,
        delay: Duration,
        v4: bool,
        v6: bool,
    }

    impl MockProvider {
        fn ok(name: &str, ip: IpAddr) -> Box<dyn Provider> {
            Box::new(Self {
                name: name.to_string(),
                ip,
                delay: Duration::ZERO,
                v4: true,
                v6: false,
            })
        }

        fn ok_delayed(name: &str, ip: IpAddr, delay: Duration) -> Box<dyn Provider> {
            Box::new(Self {
                name: name.to_string(),
                ip,
                delay,
                v4: true,
                v6: false,
            })
        }

        fn v6_only(name: &str, ip: IpAddr) -> Box<dyn Provider> {
            Box::new(Self {
                name: name.to_string(),
                ip,
                delay: Duration::ZERO,
                v4: false,
                v6: true,
            })
        }
    }

    impl Provider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }
        fn protocol(&self) -> Protocol {
            Protocol::Dns
        }
        fn supports_v4(&self) -> bool {
            self.v4
        }
        fn supports_v6(&self) -> bool {
            self.v6
        }
        fn get_ip(
            &self,
            _version: IpVersion,
        ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>> {
            let ip = self.ip;
            let delay = self.delay;
            Box::pin(async move {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                Ok(ip)
            })
        }
    }

    /// A provider that always fails.
    struct FailProvider {
        name: String,
        msg: String,
    }

    impl FailProvider {
        fn boxed(name: &str, msg: &str) -> Box<dyn Provider> {
            Box::new(Self {
                name: name.to_string(),
                msg: msg.to_string(),
            })
        }
    }

    impl Provider for FailProvider {
        fn name(&self) -> &str {
            &self.name
        }
        fn protocol(&self) -> Protocol {
            Protocol::Stun
        }
        fn get_ip(
            &self,
            _version: IpVersion,
        ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>> {
            let name = self.name.clone();
            let msg = self.msg.clone();
            Box::pin(async move { Err(ProviderError::message(name, msg)) })
        }
    }

    /// A provider that hangs forever (for timeout testing).
    struct HangProvider {
        name: String,
    }

    impl HangProvider {
        fn boxed(name: &str) -> Box<dyn Provider> {
            Box::new(Self {
                name: name.to_string(),
            })
        }
    }

    impl Provider for HangProvider {
        fn name(&self) -> &str {
            &self.name
        }
        fn protocol(&self) -> Protocol {
            Protocol::Http
        }
        fn get_ip(
            &self,
            _version: IpVersion,
        ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>> {
            Box::pin(async move {
                // Never completes
                std::future::pending().await
            })
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    fn config_with(providers: Vec<Box<dyn Provider>>, strategy: Strategy) -> Config {
        let mut builder = Config::builder()
            .strategy(strategy)
            .timeout(Duration::from_secs(2));

        for p in providers {
            builder = builder.add_provider(p);
        }
        builder.build()
    }

    // ── Strategy::First ─────────────────────────────────────────────

    #[tokio::test]
    async fn first_returns_first_success() {
        let config = config_with(
            vec![
                MockProvider::ok("A", ip(1, 1, 1, 1)),
                MockProvider::ok("B", ip(2, 2, 2, 2)),
            ],
            Strategy::First,
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(1, 1, 1, 1));
        assert_eq!(result.provider, "A");
    }

    #[tokio::test]
    async fn first_skips_failed_returns_second() {
        let config = config_with(
            vec![
                FailProvider::boxed("Bad", "down"),
                MockProvider::ok("Good", ip(3, 3, 3, 3)),
            ],
            Strategy::First,
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(3, 3, 3, 3));
        assert_eq!(result.provider, "Good");
    }

    #[tokio::test]
    async fn first_all_fail_returns_all_errors() {
        let config = config_with(
            vec![
                FailProvider::boxed("A", "err1"),
                FailProvider::boxed("B", "err2"),
            ],
            Strategy::First,
        );
        let err = Resolver::new(config).resolve().await.unwrap_err();
        match err {
            Error::AllProvidersFailed(errors) => {
                assert_eq!(errors.len(), 2);
                assert!(errors[0].to_string().contains("err1"));
                assert!(errors[1].to_string().contains("err2"));
            }
            other => panic!("expected AllProvidersFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn first_timeout_is_collected_as_error() {
        let config = config_with(
            vec![
                HangProvider::boxed("Slow"),
                MockProvider::ok("Fast", ip(4, 4, 4, 4)),
            ],
            Strategy::First,
        );
        // Should timeout on Slow, then succeed on Fast
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(4, 4, 4, 4));
    }

    #[tokio::test]
    async fn first_zero_timeout_fails_immediately() {
        let config = Config::builder()
            .timeout(Duration::ZERO)
            .strategy(Strategy::First)
            .add_provider(MockProvider::ok("Instant", ip(4, 4, 4, 4)))
            .build();

        let err = Resolver::new(config).resolve().await.unwrap_err();
        match err {
            Error::AllProvidersFailed(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(errors[0].to_string().contains("timeout"));
            }
            other => panic!("expected AllProvidersFailed, got: {other}"),
        }
    }

    // ── Strategy::Race ──────────────────────────────────────────────

    #[tokio::test]
    async fn race_returns_fastest() {
        let config = config_with(
            vec![
                MockProvider::ok_delayed("Slow", ip(1, 1, 1, 1), Duration::from_millis(100)),
                MockProvider::ok("Fast", ip(2, 2, 2, 2)),
            ],
            Strategy::Race,
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(2, 2, 2, 2));
        assert_eq!(result.provider, "Fast");
    }

    #[tokio::test]
    async fn race_with_one_failure_still_succeeds() {
        let config = config_with(
            vec![
                FailProvider::boxed("Bad", "down"),
                MockProvider::ok("Good", ip(5, 5, 5, 5)),
            ],
            Strategy::Race,
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(5, 5, 5, 5));
    }

    #[tokio::test]
    async fn race_all_fail() {
        let config = config_with(
            vec![
                FailProvider::boxed("A", "err1"),
                FailProvider::boxed("B", "err2"),
                FailProvider::boxed("C", "err3"),
            ],
            Strategy::Race,
        );
        let err = Resolver::new(config).resolve().await.unwrap_err();
        match err {
            Error::AllProvidersFailed(errors) => assert_eq!(errors.len(), 3),
            other => panic!("expected AllProvidersFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn race_timeout_plus_success() {
        let config = config_with(
            vec![
                HangProvider::boxed("Hangs"),
                MockProvider::ok("Works", ip(6, 6, 6, 6)),
            ],
            Strategy::Race,
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(6, 6, 6, 6));
    }

    #[tokio::test]
    async fn race_zero_timeout_fails_immediately() {
        let config = Config::builder()
            .timeout(Duration::ZERO)
            .strategy(Strategy::Race)
            .add_provider(MockProvider::ok("Instant", ip(6, 6, 6, 6)))
            .build();

        let err = Resolver::new(config).resolve().await.unwrap_err();
        match err {
            Error::AllProvidersFailed(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(errors[0].to_string().contains("timeout"));
            }
            other => panic!("expected AllProvidersFailed, got: {other}"),
        }
    }

    // ── Strategy::Consensus ─────────────────────────────────────────

    #[tokio::test]
    async fn consensus_reached() {
        let config = config_with(
            vec![
                MockProvider::ok("A", ip(1, 1, 1, 1)),
                MockProvider::ok("B", ip(1, 1, 1, 1)),
                MockProvider::ok("C", ip(2, 2, 2, 2)),
            ],
            Strategy::Consensus { min_agree: 2 },
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(1, 1, 1, 1));
    }

    #[tokio::test]
    async fn consensus_not_reached() {
        let config = config_with(
            vec![
                MockProvider::ok("A", ip(1, 1, 1, 1)),
                MockProvider::ok("B", ip(2, 2, 2, 2)),
                MockProvider::ok("C", ip(3, 3, 3, 3)),
            ],
            Strategy::Consensus { min_agree: 2 },
        );
        let err = Resolver::new(config).resolve().await.unwrap_err();
        match err {
            Error::ConsensusNotReached {
                required,
                got,
                errors,
            } => {
                assert_eq!(required, 2);
                assert_eq!(got, 1);
                assert!(errors.is_empty()); // all providers succeeded, just disagreed
            }
            other => panic!("expected ConsensusNotReached, got: {other}"),
        }
    }

    #[tokio::test]
    async fn consensus_errors_are_reported() {
        let config = config_with(
            vec![
                MockProvider::ok("A", ip(1, 1, 1, 1)),
                FailProvider::boxed("B", "connection refused"),
                FailProvider::boxed("C", "dns timeout"),
            ],
            Strategy::Consensus { min_agree: 2 },
        );
        let err = Resolver::new(config).resolve().await.unwrap_err();
        match err {
            Error::ConsensusNotReached {
                required,
                got,
                errors,
            } => {
                assert_eq!(required, 2);
                assert_eq!(got, 1); // only A returned an IP
                assert_eq!(errors.len(), 2); // B and C both failed
                let error_msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                assert!(error_msgs.iter().any(|m| m.contains("connection refused")));
                assert!(error_msgs.iter().any(|m| m.contains("dns timeout")));
            }
            other => panic!("expected ConsensusNotReached, got: {other}"),
        }
    }

    #[tokio::test]
    async fn consensus_zero_timeout_fails_immediately() {
        let config = Config::builder()
            .timeout(Duration::ZERO)
            .strategy(Strategy::Consensus { min_agree: 2 })
            .add_provider(MockProvider::ok("Instant", ip(1, 1, 1, 1)))
            .build();

        let err = Resolver::new(config).resolve().await.unwrap_err();
        match err {
            Error::ConsensusNotReached {
                required,
                got,
                errors,
            } => {
                assert_eq!(required, 2);
                assert_eq!(got, 0);
                assert_eq!(errors.len(), 1);
                assert!(errors[0].to_string().contains("timeout"));
            }
            other => panic!("expected ConsensusNotReached, got: {other}"),
        }
    }

    #[tokio::test]
    async fn consensus_picks_fastest_in_winning_group() {
        let config = config_with(
            vec![
                MockProvider::ok_delayed("Slow", ip(1, 1, 1, 1), Duration::from_millis(50)),
                MockProvider::ok("Fast", ip(1, 1, 1, 1)),
            ],
            Strategy::Consensus { min_agree: 2 },
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(1, 1, 1, 1));
        assert_eq!(result.provider, "Fast");
    }

    #[tokio::test]
    async fn consensus_picks_largest_group() {
        let config = config_with(
            vec![
                MockProvider::ok("A", ip(1, 1, 1, 1)),
                MockProvider::ok("B", ip(2, 2, 2, 2)),
                MockProvider::ok("C", ip(2, 2, 2, 2)),
                MockProvider::ok("D", ip(2, 2, 2, 2)),
            ],
            Strategy::Consensus { min_agree: 2 },
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        // Group {2.2.2.2: [B,C,D]} has 3 members vs {1.1.1.1: [A]} has 1
        assert_eq!(result.ip, ip(2, 2, 2, 2));
    }

    // ── Error paths ─────────────────────────────────────────────────

    #[tokio::test]
    async fn no_providers_for_version() {
        // All providers are v4-only, but we request v6
        let config = {
            let mut builder = Config::builder()
                .version(IpVersion::V6)
                .timeout(Duration::from_secs(1));
            builder = builder.add_provider(MockProvider::ok("A", ip(1, 1, 1, 1)));
            builder.build()
        };
        let err = Resolver::new(config).resolve().await.unwrap_err();
        assert!(matches!(err, Error::NoProvidersForVersion));
    }

    #[tokio::test]
    async fn v6_provider_matches_v6_request() {
        let v6 = "2001:db8::1".parse::<IpAddr>().unwrap();
        let config = {
            let mut builder = Config::builder()
                .version(IpVersion::V6)
                .timeout(Duration::from_secs(1));
            builder = builder.add_provider(MockProvider::v6_only("IPv6", v6));
            builder.build()
        };
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, v6);
    }

    #[tokio::test]
    async fn rejects_async_provider_returning_wrong_ip_family() {
        let config = {
            let mut builder = Config::builder()
                .version(IpVersion::V6)
                .timeout(Duration::from_secs(1));
            builder = builder.add_provider(MockProvider::v6_only("wrong-family", ip(1, 2, 3, 4)));
            builder.build()
        };

        let err = Resolver::new(config).resolve().await.unwrap_err();
        assert!(matches!(err, Error::AllProvidersFailed(errors) if errors.len() == 1));
    }

    // ── min_agree clamping ──────────────────────────────────────────

    #[tokio::test]
    async fn min_agree_clamped_to_2_at_build() {
        // min_agree=1 should be clamped to 2 by the builder, so a single
        // provider agreeing with itself is NOT enough for consensus.
        let config = config_with(
            vec![
                MockProvider::ok("A", ip(1, 1, 1, 1)),
                MockProvider::ok("B", ip(2, 2, 2, 2)),
            ],
            Strategy::Consensus { min_agree: 1 },
        );
        // With clamping to 2: neither IP has 2 agreements → ConsensusNotReached
        let err = Resolver::new(config).resolve().await.unwrap_err();
        assert!(matches!(err, Error::ConsensusNotReached { .. }));
    }

    #[tokio::test]
    async fn min_agree_2_passes_with_clamping() {
        // min_agree=1 is clamped to 2, and two providers agree → success
        let config = config_with(
            vec![
                MockProvider::ok("A", ip(8, 8, 8, 8)),
                MockProvider::ok("B", ip(8, 8, 8, 8)),
            ],
            Strategy::Consensus { min_agree: 1 },
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.ip, ip(8, 8, 8, 8));
    }

    // ── Latency tracking ────────────────────────────────────────────

    #[tokio::test]
    async fn latency_is_nonzero_for_delayed_provider() {
        let config = config_with(
            vec![MockProvider::ok_delayed(
                "Delayed",
                ip(1, 1, 1, 1),
                Duration::from_millis(20),
            )],
            Strategy::First,
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert!(result.latency >= Duration::from_millis(15));
    }

    // ── ConsensusNotReached Display ─────────────────────────────────

    #[test]
    fn consensus_error_display_includes_error_count() {
        let err = Error::ConsensusNotReached {
            required: 3,
            got: 1,
            errors: vec![
                ProviderError::message("A", "timeout"),
                ProviderError::message("B", "refused"),
            ],
        };
        let msg = format!("{err}");
        assert!(msg.contains("3"), "should show required count");
        assert!(msg.contains("1"), "should show got count");
        assert!(msg.contains("2 provider errors"), "should show error count");
    }

    // ── Protocol reported correctly ─────────────────────────────────

    #[tokio::test]
    async fn result_protocol_matches_provider() {
        let config = config_with(
            vec![MockProvider::ok("TestDns", ip(1, 2, 3, 4))],
            Strategy::First,
        );
        let result = Resolver::new(config).resolve().await.unwrap();
        assert_eq!(result.protocol, Protocol::Dns); // MockProvider returns Dns
    }

    #[tokio::test]
    async fn async_config_with_only_blocking_custom_provider_has_no_async_providers() {
        let config = Config::builder()
            .add_blocking_provider(MockBlockingProviderForAsyncConfig::boxed())
            .build();

        let err = Resolver::new(config).resolve().await.unwrap_err();
        assert!(matches!(err, Error::NoProvidersForVersion));
    }

    #[derive(Clone)]
    struct MockBlockingProviderForAsyncConfig;

    impl ip_discovery::BlockingProvider for MockBlockingProviderForAsyncConfig {
        fn name(&self) -> &str {
            "blocking-only"
        }
        fn protocol(&self) -> Protocol {
            Protocol::Dns
        }
        fn get_ip(&self, _version: IpVersion, _timeout: Duration) -> Result<IpAddr, ProviderError> {
            Ok(ip(1, 1, 1, 1))
        }
        fn clone_box(&self) -> ip_discovery::BoxedBlockingProvider {
            Box::new(self.clone())
        }
    }

    impl MockBlockingProviderForAsyncConfig {
        fn boxed() -> ip_discovery::BoxedBlockingProvider {
            Box::new(Self)
        }
    }
}

#[cfg(test)]
mod blocking_resolver_tests {
    use ip_discovery::blocking::Resolver;
    use ip_discovery::{
        BlockingProvider, BoxedBlockingProvider, Config, Error, IpVersion, Protocol, ProviderError,
        Strategy,
    };
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    struct MockBlockingProvider {
        name: String,
        ip: IpAddr,
        delay: Duration,
        v4: bool,
        v6: bool,
    }

    impl MockBlockingProvider {
        fn ok(name: &str, ip: IpAddr) -> BoxedBlockingProvider {
            Box::new(Self {
                name: name.to_string(),
                ip,
                delay: Duration::ZERO,
                v4: true,
                v6: false,
            })
        }

        fn ok_delayed(name: &str, ip: IpAddr, delay: Duration) -> BoxedBlockingProvider {
            Box::new(Self {
                name: name.to_string(),
                ip,
                delay,
                v4: true,
                v6: false,
            })
        }
    }

    impl BlockingProvider for MockBlockingProvider {
        fn name(&self) -> &str {
            &self.name
        }
        fn protocol(&self) -> Protocol {
            Protocol::Dns
        }
        fn supports_v4(&self) -> bool {
            self.v4
        }
        fn supports_v6(&self) -> bool {
            self.v6
        }
        fn get_ip(&self, _version: IpVersion, _timeout: Duration) -> Result<IpAddr, ProviderError> {
            if !self.delay.is_zero() {
                std::thread::sleep(self.delay);
            }
            Ok(self.ip)
        }
        fn clone_box(&self) -> BoxedBlockingProvider {
            Box::new(Self {
                name: self.name.clone(),
                ip: self.ip,
                delay: self.delay,
                v4: self.v4,
                v6: self.v6,
            })
        }
    }

    struct FailBlockingProvider {
        name: String,
        msg: String,
    }

    #[derive(Clone)]
    struct PanicBlockingProvider;

    impl BlockingProvider for PanicBlockingProvider {
        fn name(&self) -> &str {
            "panics"
        }
        fn protocol(&self) -> Protocol {
            Protocol::Dns
        }
        fn get_ip(&self, _version: IpVersion, _timeout: Duration) -> Result<IpAddr, ProviderError> {
            panic!("intentional provider panic")
        }
        fn clone_box(&self) -> BoxedBlockingProvider {
            Box::new(self.clone())
        }
    }

    impl FailBlockingProvider {
        fn boxed(name: &str, msg: &str) -> BoxedBlockingProvider {
            Box::new(Self {
                name: name.to_string(),
                msg: msg.to_string(),
            })
        }
    }

    impl BlockingProvider for FailBlockingProvider {
        fn name(&self) -> &str {
            &self.name
        }
        fn protocol(&self) -> Protocol {
            Protocol::Dns
        }
        fn get_ip(&self, _version: IpVersion, _timeout: Duration) -> Result<IpAddr, ProviderError> {
            Err(ProviderError::message(&self.name, &self.msg))
        }
        fn clone_box(&self) -> BoxedBlockingProvider {
            Box::new(Self {
                name: self.name.clone(),
                msg: self.msg.clone(),
            })
        }
    }

    #[test]
    fn test_blocking_first_success() {
        let ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let config = Config::builder()
            .add_blocking_provider(MockBlockingProvider::ok("p1", ip))
            .build();
        let res = Resolver::new(config).resolve().unwrap();
        assert_eq!(res.ip, ip);
        assert_eq!(res.provider, "p1");
    }

    #[test]
    fn test_blocking_first_fallback() {
        let ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let config = Config::builder()
            .add_blocking_provider(FailBlockingProvider::boxed("p1", "err"))
            .add_blocking_provider(MockBlockingProvider::ok("p2", ip))
            .build();
        let res = Resolver::new(config).resolve().unwrap();
        assert_eq!(res.ip, ip);
        assert_eq!(res.provider, "p2");
    }

    #[test]
    fn test_blocking_all_failed() {
        let config = Config::builder()
            .add_blocking_provider(FailBlockingProvider::boxed("p1", "err1"))
            .add_blocking_provider(FailBlockingProvider::boxed("p2", "err2"))
            .build();
        let res = Resolver::new(config).resolve();
        assert!(matches!(res, Err(Error::AllProvidersFailed(_))));
    }

    #[test]
    fn test_blocking_provider_panic_becomes_provider_error() {
        let config = Config::builder()
            .timeout(Duration::from_millis(100))
            .add_blocking_provider(Box::new(PanicBlockingProvider))
            .build();

        let err = Resolver::new(config).resolve().unwrap_err();
        match err {
            Error::AllProvidersFailed(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(errors[0].to_string().contains("provider panicked"));
            }
            other => panic!("expected AllProvidersFailed, got {other}"),
        }
    }

    #[test]
    fn test_blocking_race() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = Config::builder()
            .strategy(Strategy::Race)
            .add_blocking_provider(MockBlockingProvider::ok_delayed(
                "slow",
                ip1,
                Duration::from_millis(50),
            ))
            .add_blocking_provider(MockBlockingProvider::ok("fast", ip2))
            .build();
        let res = Resolver::new(config).resolve().unwrap();
        assert_eq!(res.ip, ip2);
        assert_eq!(res.provider, "fast");
    }

    #[test]
    fn test_blocking_consensus() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = Config::builder()
            .strategy(Strategy::Consensus { min_agree: 2 })
            .add_blocking_provider(MockBlockingProvider::ok("p1", ip1))
            .add_blocking_provider(MockBlockingProvider::ok("p2", ip1))
            .add_blocking_provider(MockBlockingProvider::ok("p3", ip2))
            .build();
        let res = Resolver::new(config).resolve().unwrap();
        assert_eq!(res.ip, ip1);
    }

    #[test]
    fn test_blocking_first_enforces_timeout_and_falls_back() {
        let slow_ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let fast_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = Config::builder()
            .timeout(Duration::from_millis(10))
            .add_blocking_provider(MockBlockingProvider::ok_delayed(
                "ignores-timeout",
                slow_ip,
                Duration::from_millis(150),
            ))
            .add_blocking_provider(MockBlockingProvider::ok("fallback", fast_ip))
            .build();

        let started = std::time::Instant::now();
        let result = Resolver::new(config).resolve().unwrap();

        assert_eq!(result.provider, "fallback");
        assert_eq!(result.ip, fast_ip);
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn test_blocking_race_enforces_overall_timeout() {
        let ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let config = Config::builder()
            .strategy(Strategy::Race)
            .timeout(Duration::from_millis(10))
            .add_blocking_provider(MockBlockingProvider::ok_delayed(
                "slow-a",
                ip,
                Duration::from_millis(150),
            ))
            .add_blocking_provider(MockBlockingProvider::ok_delayed(
                "slow-b",
                ip,
                Duration::from_millis(150),
            ))
            .build();

        let started = std::time::Instant::now();
        let result = Resolver::new(config).resolve();

        assert!(matches!(result, Err(Error::AllProvidersFailed(errors)) if errors.len() == 2));
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn test_blocking_consensus_uses_results_received_before_deadline() {
        let winning_ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let slow_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = Config::builder()
            .strategy(Strategy::Consensus { min_agree: 2 })
            .timeout(Duration::from_millis(20))
            .add_blocking_provider(MockBlockingProvider::ok("fast-a", winning_ip))
            .add_blocking_provider(MockBlockingProvider::ok("fast-b", winning_ip))
            .add_blocking_provider(MockBlockingProvider::ok_delayed(
                "ignores-timeout",
                slow_ip,
                Duration::from_millis(150),
            ))
            .build();

        let started = std::time::Instant::now();
        let result = Resolver::new(config).resolve().unwrap();

        assert_eq!(result.ip, winning_ip);
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn rejects_blocking_provider_returning_wrong_ip_family() {
        let config = Config::builder()
            .version(IpVersion::V6)
            .add_blocking_provider(Box::new(MockBlockingProvider {
                name: "wrong-family".to_string(),
                ip: IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
                delay: Duration::ZERO,
                v4: false,
                v6: true,
            }))
            .build();

        let err = Resolver::new(config).resolve().unwrap_err();
        assert!(matches!(err, Error::AllProvidersFailed(errors) if errors.len() == 1));
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn blocking_config_with_only_async_custom_provider_has_no_blocking_providers() {
        let config = Config::builder()
            .add_provider(AsyncOnlyProvider::boxed())
            .build();

        let err = Resolver::new(config).resolve().unwrap_err();
        assert!(matches!(err, Error::NoProvidersForVersion));
    }

    #[cfg(feature = "tokio")]
    struct AsyncOnlyProvider;

    #[cfg(feature = "tokio")]
    impl ip_discovery::Provider for AsyncOnlyProvider {
        fn name(&self) -> &str {
            "async-only"
        }
        fn protocol(&self) -> Protocol {
            Protocol::Dns
        }
        fn get_ip(
            &self,
            _version: IpVersion,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<IpAddr, ProviderError>> + Send + '_>,
        > {
            Box::pin(async { Ok(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))) })
        }
    }

    #[cfg(feature = "tokio")]
    impl AsyncOnlyProvider {
        fn boxed() -> ip_discovery::BoxedProvider {
            Box::new(Self)
        }
    }
}
