//! Unit tests for config, error, HTTP parser, and types modules

#[cfg(test)]
mod config_tests {
    use ip_discovery::{BuiltinProvider, Config, IpVersion, Protocol, Strategy};
    use std::time::Duration;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        let _ = config;
    }

    #[test]
    fn test_builder_default() {
        let config = Config::builder().build();
        let _ = config;
    }

    #[test]
    fn test_builder_timeout() {
        let config = Config::builder().timeout(Duration::from_secs(3)).build();
        let _ = config;
    }

    #[test]
    fn test_builder_version_v4() {
        let config = Config::builder().version(IpVersion::V4).build();
        let _ = config;
    }

    #[test]
    fn test_builder_version_v6() {
        let config = Config::builder().version(IpVersion::V6).build();
        let _ = config;
    }

    #[test]
    fn test_builder_strategy_race() {
        let config = Config::builder().strategy(Strategy::Race).build();
        let _ = config;
    }

    #[test]
    fn test_builder_strategy_consensus() {
        let config = Config::builder()
            .strategy(Strategy::Consensus { min_agree: 3 })
            .build();
        let _ = config;
    }

    #[test]
    fn test_builder_protocol_filter() {
        let config = Config::builder().protocols(&[Protocol::Dns]).build();
        let _ = config;
    }

    #[test]
    fn test_builder_specific_providers() {
        let config = Config::builder()
            .providers(&[BuiltinProvider::CloudflareDns, BuiltinProvider::GoogleStun])
            .build();
        let _ = config;
    }

    #[test]
    fn test_builder_all_options() {
        let config = Config::builder()
            .protocols(&[Protocol::Stun, Protocol::Dns])
            .strategy(Strategy::Race)
            .version(IpVersion::V4)
            .timeout(Duration::from_secs(5))
            .build();
        let _ = config;
    }

    #[test]
    fn test_builder_chained_multiple_protocols() {
        let config = Config::builder()
            .protocols(&[Protocol::Dns, Protocol::Stun, Protocol::Http])
            .version(IpVersion::Any)
            .strategy(Strategy::First)
            .build();
        let _ = config;
    }

    #[test]
    fn test_builder_very_short_timeout() {
        let config = Config::builder().timeout(Duration::from_millis(1)).build();
        let _ = config;
    }

    #[test]
    fn test_builder_consensus_min_agree_1() {
        // min_agree=1 is valid to set, the resolver clamps it to 2
        let config = Config::builder()
            .strategy(Strategy::Consensus { min_agree: 1 })
            .build();
        let _ = config;
    }
}

#[cfg(test)]
mod error_tests {
    use ip_discovery::{Error, ProviderError};

    #[test]
    fn test_error_display_no_version() {
        let err = Error::NoProvidersForVersion;
        assert!(format!("{}", err).contains("IP version"));
    }

    #[test]
    fn test_error_display_consensus() {
        let err = Error::ConsensusNotReached {
            required: 3,
            got: 1,
            errors: vec![],
        };
        let msg = format!("{}", err);
        assert!(msg.contains("3"));
        assert!(msg.contains("1"));
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::message("TestProvider", "connection refused");
        let msg = format!("{}", err);
        assert!(msg.contains("TestProvider"));
        assert!(msg.contains("connection refused"));
    }

    #[test]
    fn test_provider_error_new() {
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
        let err = ProviderError::new("Google DNS", io_err);
        let msg = format!("{}", err);
        assert!(msg.contains("Google DNS"));
    }

    #[test]
    fn test_all_providers_failed() {
        let errors = vec![
            ProviderError::message("A", "fail1"),
            ProviderError::message("B", "fail2"),
        ];
        let err = Error::AllProvidersFailed(errors);
        let msg = format!("{}", err);
        assert!(msg.contains("all providers failed"));
    }

    #[test]
    fn test_consensus_not_reached_zero() {
        let err = Error::ConsensusNotReached {
            required: 2,
            got: 0,
            errors: vec![],
        };
        let msg = format!("{}", err);
        assert!(msg.contains("0"));
    }

    #[test]
    fn test_provider_error_debug_impl() {
        let err = ProviderError::message("Test", "err");
        let debug = format!("{:?}", err);
        assert!(!debug.is_empty());
    }
}

#[cfg(test)]
mod types_tests {
    use ip_discovery::{BuiltinProvider, IpVersion, Protocol, ProviderResult};

    #[test]
    fn test_protocol_display() {
        assert_eq!(format!("{}", Protocol::Dns), "DNS");
        assert_eq!(format!("{}", Protocol::Http), "HTTP");
        assert_eq!(format!("{}", Protocol::Stun), "STUN");
    }

    #[test]
    fn test_builtin_provider_protocol() {
        assert_eq!(BuiltinProvider::GoogleStun.protocol(), Protocol::Stun);
        assert_eq!(BuiltinProvider::CloudflareDns.protocol(), Protocol::Dns);
        assert_eq!(BuiltinProvider::Aws.protocol(), Protocol::Http);
    }

    #[test]
    fn test_builtin_provider_display() {
        assert_eq!(format!("{}", BuiltinProvider::GoogleStun), "Google STUN");
        assert_eq!(
            format!("{}", BuiltinProvider::CloudflareDns),
            "Cloudflare DNS"
        );
        assert_eq!(format!("{}", BuiltinProvider::Aws), "AWS");
    }

    #[test]
    fn test_builtin_provider_all() {
        assert_eq!(BuiltinProvider::ALL.len(), 9);
    }

    #[test]
    fn test_ip_version_default() {
        let version = IpVersion::default();
        assert_eq!(version, IpVersion::Any);
    }

    #[test]
    fn test_all_builtin_providers_have_nonempty_display() {
        for provider in BuiltinProvider::ALL {
            let name = format!("{}", provider);
            assert!(
                !name.is_empty(),
                "Provider {:?} has empty display name",
                provider
            );
        }
    }

    #[test]
    fn test_all_builtin_providers_have_protocol() {
        for provider in BuiltinProvider::ALL {
            let protocol = provider.protocol();
            // Every provider must belong to one of the three protocols
            assert!(
                matches!(protocol, Protocol::Dns | Protocol::Http | Protocol::Stun),
                "Provider {:?} has unexpected protocol {:?}",
                provider,
                protocol
            );
        }
    }

    #[test]
    fn test_ip_version_variants() {
        let _ = IpVersion::V4;
        let _ = IpVersion::V6;
        let _ = IpVersion::Any;
    }

    #[test]
    fn test_protocol_debug_impl() {
        let debug = format!("{:?}", Protocol::Dns);
        assert!(debug.contains("Dns"));
    }

    #[test]
    fn test_protocol_from_str() {
        use std::str::FromStr;
        assert_eq!(Protocol::from_str("dns"), Ok(Protocol::Dns));
        assert_eq!(Protocol::from_str("DNS"), Ok(Protocol::Dns));
        assert_eq!(Protocol::from_str("stun"), Ok(Protocol::Stun));
        assert_eq!(Protocol::from_str("STUN"), Ok(Protocol::Stun));
        assert_eq!(Protocol::from_str("http"), Ok(Protocol::Http));
        assert_eq!(Protocol::from_str("HTTP"), Ok(Protocol::Http));
        assert_eq!(Protocol::from_str("https"), Ok(Protocol::Http));
        assert!(Protocol::from_str("unknown").is_err());
    }

    #[test]
    fn test_ip_version_from_str() {
        use std::str::FromStr;
        assert_eq!(IpVersion::from_str("v4"), Ok(IpVersion::V4));
        assert_eq!(IpVersion::from_str("4"), Ok(IpVersion::V4));
        assert_eq!(IpVersion::from_str("ipv4"), Ok(IpVersion::V4));
        assert_eq!(IpVersion::from_str("v6"), Ok(IpVersion::V6));
        assert_eq!(IpVersion::from_str("6"), Ok(IpVersion::V6));
        assert_eq!(IpVersion::from_str("ipv6"), Ok(IpVersion::V6));
        assert_eq!(IpVersion::from_str("any"), Ok(IpVersion::Any));
        assert!(IpVersion::from_str("v5").is_err());
    }

    #[test]
    fn test_builtin_provider_from_str() {
        use std::str::FromStr;
        assert_eq!(
            BuiltinProvider::from_str("google-stun"),
            Ok(BuiltinProvider::GoogleStun)
        );
        assert_eq!(
            BuiltinProvider::from_str("google-stun1"),
            Ok(BuiltinProvider::GoogleStun1)
        );
        assert_eq!(
            BuiltinProvider::from_str("google-stun2"),
            Ok(BuiltinProvider::GoogleStun2)
        );
        assert_eq!(
            BuiltinProvider::from_str("cloudflare-stun"),
            Ok(BuiltinProvider::CloudflareStun)
        );
        assert_eq!(
            BuiltinProvider::from_str("google-dns"),
            Ok(BuiltinProvider::GoogleDns)
        );
        assert_eq!(
            BuiltinProvider::from_str("cloudflare-dns"),
            Ok(BuiltinProvider::CloudflareDns)
        );
        assert_eq!(
            BuiltinProvider::from_str("opendns"),
            Ok(BuiltinProvider::OpenDns)
        );
        assert_eq!(
            BuiltinProvider::from_str("cloudflare-http"),
            Ok(BuiltinProvider::CloudflareHttp)
        );
        assert_eq!(BuiltinProvider::from_str("aws"), Ok(BuiltinProvider::Aws));
        assert_eq!(
            BuiltinProvider::from_str("GOOGLE_DNS"),
            Ok(BuiltinProvider::GoogleDns)
        );
        assert!(BuiltinProvider::from_str("invalid-provider").is_err());
    }

    #[test]
    fn test_provider_result_ip_helpers() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
        use std::time::Duration;

        let v4_res = ProviderResult {
            ip: IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            provider: "test".into(),
            protocol: Protocol::Dns,
            latency: Duration::from_millis(10),
        };
        assert!(v4_res.is_ipv4());
        assert!(!v4_res.is_ipv6());

        let v6_res = ProviderResult {
            ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
            provider: "test".into(),
            protocol: Protocol::Dns,
            latency: Duration::from_millis(10),
        };
        assert!(!v6_res.is_ipv4());
        assert!(v6_res.is_ipv6());
    }
}

#[cfg(feature = "http")]
#[cfg(test)]
mod http_tests {
    use ip_discovery::http::{parse_cloudflare_trace, parse_plain_text, HttpProvider};
    use ip_discovery::IpVersion;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    // ── parse_plain_text ────────────────────────────────────────────

    #[test]
    fn test_parse_plain_text_ipv4() {
        let ip = parse_plain_text("1.2.3.4\n");
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "1.2.3.4");
    }

    #[test]
    fn test_parse_plain_text_ipv6() {
        let ip = parse_plain_text("2001:db8::1\n");
        assert!(ip.is_some());
    }

    #[test]
    fn test_parse_plain_text_invalid() {
        let ip = parse_plain_text("not an ip");
        assert!(ip.is_none());
    }

    #[test]
    fn test_parse_plain_text_empty() {
        let ip = parse_plain_text("");
        assert!(ip.is_none());
    }

    #[test]
    fn test_parse_plain_text_with_whitespace() {
        let ip = parse_plain_text("  203.0.113.1  \n");
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "203.0.113.1");
    }

    #[test]
    fn test_parse_plain_text_crlf() {
        let ip = parse_plain_text("10.0.0.1\r\n");
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "10.0.0.1");
    }

    #[test]
    fn test_parse_plain_text_ipv4_mapped_ipv6() {
        let ip = parse_plain_text("::ffff:192.168.1.1\n");
        assert!(ip.is_some());
    }

    #[test]
    fn test_parse_plain_text_loopback() {
        let ip = parse_plain_text("127.0.0.1");
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "127.0.0.1");
    }

    #[test]
    fn test_parse_plain_text_ipv6_full() {
        let ip = parse_plain_text("2001:0db8:0000:0000:0000:0000:0000:0001");
        assert!(ip.is_some());
    }

    #[test]
    fn test_parse_plain_text_garbage() {
        assert!(parse_plain_text("hello world").is_none());
        assert!(parse_plain_text("1.2.3.999").is_none());
        assert!(parse_plain_text("1.2.3").is_none());
        assert!(parse_plain_text(":::").is_none());
    }

    #[test]
    fn test_parse_plain_text_multiple_lines() {
        // Should only parse the trimmed full text, not individual lines
        let ip = parse_plain_text("1.2.3.4\n5.6.7.8\n");
        assert!(ip.is_none()); // "1.2.3.4\n5.6.7.8" trimmed is not a valid IP
    }

    // ── parse_cloudflare_trace ──────────────────────────────────────

    #[test]
    fn test_parse_cloudflare_trace() {
        let response = "fl=123abc\nip=203.0.113.1\nts=1234567890\n";
        let ip = parse_cloudflare_trace(response);
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "203.0.113.1");
    }

    #[test]
    fn test_parse_cloudflare_trace_no_ip() {
        let response = "fl=123abc\nts=1234567890\n";
        let ip = parse_cloudflare_trace(response);
        assert!(ip.is_none());
    }

    #[test]
    fn test_parse_cloudflare_trace_ipv6() {
        let response = "fl=abc\nip=2001:db8::1\nts=123\n";
        let ip = parse_cloudflare_trace(response);
        assert!(ip.is_some());
    }

    #[test]
    fn test_parse_cloudflare_trace_ip_first_line() {
        let response = "ip=10.0.0.1\nfl=abc\n";
        let ip = parse_cloudflare_trace(response);
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "10.0.0.1");
    }

    #[test]
    fn test_parse_cloudflare_trace_ip_last_line() {
        let response = "fl=abc\nts=123\nip=172.16.0.1";
        let ip = parse_cloudflare_trace(response);
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "172.16.0.1");
    }

    #[test]
    fn test_parse_cloudflare_trace_empty() {
        assert!(parse_cloudflare_trace("").is_none());
    }

    #[test]
    fn test_parse_cloudflare_trace_ip_prefix_only() {
        // "ip=" with no value
        assert!(parse_cloudflare_trace("ip=\n").is_none());
    }

    #[test]
    fn test_parse_cloudflare_trace_ip_invalid_value() {
        assert!(parse_cloudflare_trace("ip=not_an_ip\n").is_none());
    }

    #[test]
    fn test_parse_cloudflare_trace_similar_key() {
        // "ipaddr=" should not match "ip="
        let response = "ipaddr=1.2.3.4\nfl=abc\n";
        assert!(parse_cloudflare_trace(response).is_none());
    }

    #[test]
    fn test_parse_cloudflare_trace_crlf() {
        let response = "fl=abc\r\nip=8.8.8.8\r\nts=123\r\n";
        let ip = parse_cloudflare_trace(response);
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "8.8.8.8");
    }

    #[test]
    fn test_parse_cloudflare_trace_full_realistic() {
        let response = "\
fl=638f123\n\
h=1.1.1.1\n\
ip=198.51.100.42\n\
ts=1710000000.123\n\
visit_scheme=https\n\
uag=ip-discovery/0.1.5\n\
colo=LAX\n\
sliver=none\n\
http=http/2\n\
loc=US\n\
tls=TLSv1.3\n\
sni=plaintext\n\
warp=off\n\
gateway=off\n\
rbi=off\n\
kex=X25519\n";
        let ip = parse_cloudflare_trace(response);
        assert!(ip.is_some());
        assert_eq!(ip.unwrap().to_string(), "198.51.100.42");
    }

    #[test]
    fn blocking_http_rejects_wrong_ip_family() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n203.0.113.1")
                .unwrap();
        });

        let provider = HttpProvider::new("local-http", &url).with_v6_url(url);
        let result = provider.fetch_blocking(IpVersion::V6, Duration::from_secs(1));

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod blocking_api_tests {
    use ip_discovery::blocking::{get_ip_with, Resolver};
    use ip_discovery::{
        BlockingProvider, BoxedBlockingProvider, Config, Error, IpVersion, Protocol, ProviderError,
    };
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    #[derive(Clone)]
    struct DummyProvider {
        name: String,
        ip: Option<IpAddr>,
    }

    impl BlockingProvider for DummyProvider {
        fn name(&self) -> &str {
            &self.name
        }
        fn protocol(&self) -> Protocol {
            Protocol::Dns
        }
        fn get_ip(&self, _version: IpVersion, _timeout: Duration) -> Result<IpAddr, ProviderError> {
            self.ip
                .ok_or_else(|| ProviderError::message(&self.name, "no ip"))
        }
        fn clone_box(&self) -> BoxedBlockingProvider {
            Box::new(self.clone())
        }
    }

    #[test]
    fn test_blocking_provider_defaults() {
        let p = DummyProvider {
            name: "test".to_string(),
            ip: None,
        };
        assert!(p.supports_v4());
        assert!(!p.supports_v6());
        assert!(p.supports_version(IpVersion::V4));
        assert!(!p.supports_version(IpVersion::V6));
        assert!(p.supports_version(IpVersion::Any));
    }

    #[test]
    fn test_blocking_empty_providers() {
        let config = Config::builder().protocols(&[]).build();
        let res = get_ip_with(config);
        assert!(matches!(res, Err(Error::NoProvidersForVersion)));
    }

    #[test]
    fn test_blocking_resolver_new_and_resolve() {
        let ip = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
        let config = Config::builder()
            .add_blocking_provider(Box::new(DummyProvider {
                name: "dummy".to_string(),
                ip: Some(ip),
            }))
            .build();
        let resolver = Resolver::new(config);
        let res = resolver.resolve().unwrap();
        assert_eq!(res.ip, ip);
        assert_eq!(res.provider, "dummy");
        assert_eq!(res.protocol, Protocol::Dns);
    }
}
