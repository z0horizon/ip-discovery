//! Synchronous, blocking IP detection engine.
//!
//! This module provides a zero-runtime-dependency synchronous API for resolving
//! public IP addresses using standard library UDP sockets and threads.
//!
//! Blocking calls return when their configured deadline is reached. Providers
//! are executed on worker threads so a non-cooperative custom provider cannot
//! block the caller indefinitely. Such provider code cannot be forcibly
//! cancelled and may finish later in the background, so custom providers should
//! still honor the timeout passed to [`BlockingProvider::get_ip`](crate::BlockingProvider::get_ip).
//!
//! # Examples
//!
//! ```rust,no_run
//! use ip_discovery::blocking::{get_ip, get_ipv4};
//!
//! // Get any public IP
//! if let Ok(result) = get_ip() {
//!     println!("Public IP: {} (via {})", result.ip, result.provider);
//! }
//!
//! // Get IPv4 specifically
//! if let Ok(result) = get_ipv4() {
//!     println!("IPv4: {}", result.ip);
//! }
//! ```

use crate::config::{Config, Strategy};
use crate::error::Error;
use crate::provider::BoxedBlockingProvider;
use crate::types::{IpVersion, Protocol, ProviderResult};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

type WorkerMessage = (
    usize,
    String,
    Protocol,
    Result<IpAddr, crate::ProviderError>,
    Duration,
);

fn spawn_provider_worker(
    id: usize,
    provider: BoxedBlockingProvider,
    version: IpVersion,
    timeout: Duration,
    tx: Sender<WorkerMessage>,
) -> Result<(), crate::ProviderError> {
    let name = provider.name().to_string();
    let spawn_error_name = name.clone();
    let protocol = provider.protocol();
    let thread_name = format!("ip-discovery-{id}");
    std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let start = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                provider.get_ip(version, timeout)
            }))
            .unwrap_or_else(|_| Err(crate::ProviderError::message(&name, "provider panicked")));
            let latency = start.elapsed();
            let _ = tx.send((id, name, protocol, result, latency));
        })
        .map(|_| ())
        .map_err(|error| crate::ProviderError::new(spawn_error_name, error))
}

fn timeout_error(name: impl Into<String>) -> crate::ProviderError {
    crate::ProviderError::message(name, "timeout")
}

fn validate_ip(
    name: impl Into<String>,
    version: IpVersion,
    ip: IpAddr,
) -> Result<IpAddr, crate::ProviderError> {
    if version.matches(ip) {
        Ok(ip)
    } else {
        Err(crate::ProviderError::message(
            name,
            "provider returned unexpected IP version",
        ))
    }
}

fn receive_with_timeout(
    rx: &Receiver<WorkerMessage>,
    timeout: Duration,
) -> Result<WorkerMessage, RecvTimeoutError> {
    rx.recv_timeout(timeout)
}

/// Coordinates synchronous IP detection across configured providers.
///
/// Created via [`Resolver::new()`] with a [`Config`].
/// Call [`resolve()`](Resolver::resolve) to perform the lookup.
pub struct Resolver {
    config: Config,
}

impl Resolver {
    /// Create a new blocking resolver with the given configuration
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Return an iterator over blocking providers that support the configured IP version.
    #[inline]
    fn matching_providers(&self) -> impl Iterator<Item = &BoxedBlockingProvider> {
        self.config
            .blocking_providers
            .iter()
            .filter(|p| p.supports_version(self.config.version))
    }

    /// Resolve the public IP address synchronously using the configured strategy.
    ///
    /// # Errors
    ///
    /// - [`Error::NoProvidersForVersion`] — no provider supports the requested IP version.
    /// - [`Error::AllProvidersFailed`] — every provider either failed or timed out.
    /// - [`Error::ConsensusNotReached`] — (consensus strategy) too few providers agreed.
    pub fn resolve(&self) -> Result<ProviderResult, Error> {
        if self.matching_providers().next().is_none() {
            return Err(Error::NoProvidersForVersion);
        }

        match self.config.strategy {
            Strategy::First => self.resolve_first(),
            Strategy::Race => self.resolve_race(),
            Strategy::Consensus { min_agree } => self.resolve_consensus(min_agree),
        }
    }

    /// Try providers sequentially in order, returning the first success.
    fn resolve_first(&self) -> Result<ProviderResult, Error> {
        let mut errors = Vec::new();

        for (id, provider) in self.matching_providers().enumerate() {
            let provider_name = provider.name().to_string();
            if self.config.timeout.is_zero() {
                errors.push(timeout_error(provider_name));
                continue;
            }

            let (tx, rx) = std::sync::mpsc::channel();
            if let Err(error) = spawn_provider_worker(
                id,
                provider.clone(),
                self.config.version,
                self.config.timeout,
                tx,
            ) {
                errors.push(error);
                continue;
            }

            match receive_with_timeout(&rx, self.config.timeout) {
                Ok((_, name, protocol, Ok(ip), latency)) => {
                    match validate_ip(&name, self.config.version, ip) {
                        Ok(ip) => {
                            return Ok(ProviderResult {
                                ip,
                                provider: name,
                                protocol,
                                latency,
                            });
                        }
                        Err(error) => errors.push(error),
                    }
                }
                Ok((_, _, _, Err(error), _)) => errors.push(error),
                Err(RecvTimeoutError::Timeout) => errors.push(timeout_error(provider_name)),
                Err(RecvTimeoutError::Disconnected) => errors.push(crate::ProviderError::message(
                    provider_name,
                    "provider worker disconnected",
                )),
            }
        }

        Err(Error::AllProvidersFailed(errors))
    }

    /// Race all providers concurrently across threads, returning the fastest success.
    ///
    /// Workers that have already started cannot be forcibly cancelled when a
    /// winner is found. Built-in providers use bounded I/O; custom providers
    /// should honor their timeout to release background resources promptly.
    fn resolve_race(&self) -> Result<ProviderResult, Error> {
        let providers: Vec<BoxedBlockingProvider> = self.matching_providers().cloned().collect();
        if providers.is_empty() {
            return Err(Error::NoProvidersForVersion);
        }

        if self.config.timeout.is_zero() {
            return Err(Error::AllProvidersFailed(
                providers
                    .iter()
                    .map(|provider| timeout_error(provider.name()))
                    .collect(),
            ));
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut pending = HashMap::new();
        let mut errors = Vec::new();

        for (id, provider) in providers.into_iter().enumerate() {
            pending.insert(id, provider.name().to_string());
            if let Err(error) = spawn_provider_worker(
                id,
                provider,
                self.config.version,
                self.config.timeout,
                tx.clone(),
            ) {
                pending.remove(&id);
                errors.push(error);
            }
        }
        drop(tx);

        let deadline = Instant::now().checked_add(self.config.timeout);

        while !pending.is_empty() {
            let remaining = deadline
                .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                break;
            }
            match receive_with_timeout(&rx, remaining) {
                Ok((id, name, protocol, res, latency)) => {
                    pending.remove(&id);
                    match res {
                        Ok(ip) => match validate_ip(&name, self.config.version, ip) {
                            Ok(ip) => {
                                return Ok(ProviderResult {
                                    ip,
                                    provider: name,
                                    protocol,
                                    latency,
                                });
                            }
                            Err(error) => errors.push(error),
                        },
                        Err(e) => errors.push(e),
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }

        errors.extend(pending.into_values().map(timeout_error));

        Err(Error::AllProvidersFailed(errors))
    }

    /// Query all providers concurrently across threads and require consensus.
    fn resolve_consensus(&self, min_agree: usize) -> Result<ProviderResult, Error> {
        let providers: Vec<BoxedBlockingProvider> = self.matching_providers().cloned().collect();
        if providers.is_empty() {
            return Err(Error::NoProvidersForVersion);
        }

        if self.config.timeout.is_zero() {
            return Err(Error::ConsensusNotReached {
                required: min_agree,
                got: 0,
                errors: providers
                    .iter()
                    .map(|provider| timeout_error(provider.name()))
                    .collect(),
            });
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut pending = HashMap::new();
        let mut errors = Vec::new();

        for (id, provider) in providers.into_iter().enumerate() {
            pending.insert(id, provider.name().to_string());
            if let Err(error) = spawn_provider_worker(
                id,
                provider,
                self.config.version,
                self.config.timeout,
                tx.clone(),
            ) {
                pending.remove(&id);
                errors.push(error);
            }
        }
        drop(tx);

        let mut ip_results: HashMap<IpAddr, Vec<ProviderResult>> = HashMap::new();
        let deadline = Instant::now().checked_add(self.config.timeout);

        while !pending.is_empty() {
            let remaining = deadline
                .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                break;
            }
            match receive_with_timeout(&rx, remaining) {
                Ok((id, name, protocol, res, latency)) => {
                    pending.remove(&id);
                    match res {
                        Ok(ip) => match validate_ip(&name, self.config.version, ip) {
                            Ok(ip) => {
                                let pr = ProviderResult {
                                    ip,
                                    provider: name,
                                    protocol,
                                    latency,
                                };
                                ip_results.entry(ip).or_default().push(pr);
                            }
                            Err(error) => errors.push(error),
                        },
                        Err(e) => errors.push(e),
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }

        errors.extend(pending.into_values().map(timeout_error));

        let mut best: Option<(IpAddr, usize)> = None;
        for (ip, providers) in &ip_results {
            if providers.len() >= min_agree {
                match &best {
                    None => best = Some((*ip, providers.len())),
                    Some((_, current_len)) if providers.len() > *current_len => {
                        best = Some((*ip, providers.len()))
                    }
                    _ => {}
                }
            }
        }

        match best {
            Some((ip, _)) => {
                if let Some(providers) = ip_results.remove(&ip) {
                    if let Some(fastest) = providers.into_iter().min_by_key(|p| p.latency) {
                        return Ok(fastest);
                    }
                }
                Err(Error::ConsensusNotReached {
                    required: min_agree,
                    got: 0,
                    errors,
                })
            }
            None => {
                let max_agreement = ip_results.values().map(|v| v.len()).max().unwrap_or(0);
                Err(Error::ConsensusNotReached {
                    required: min_agree,
                    got: max_agreement,
                    errors,
                })
            }
        }
    }
}

/// Get public IP address synchronously using default configuration.
///
/// # Errors
///
/// Returns [`Error::AllProvidersFailed`] if every provider fails.
pub fn get_ip() -> Result<ProviderResult, Error> {
    let config = Config::default();
    get_ip_with(config)
}

/// Get public IPv4 address synchronously using default configuration.
///
/// # Errors
///
/// Returns [`Error::AllProvidersFailed`] if no provider returns an IPv4 address.
pub fn get_ipv4() -> Result<ProviderResult, Error> {
    let config = Config::builder().version(IpVersion::V4).build();
    get_ip_with(config)
}

/// Get public IPv6 address synchronously using default configuration.
///
/// # Errors
///
/// Returns [`Error::NoProvidersForVersion`] if no provider supports IPv6.
pub fn get_ipv6() -> Result<ProviderResult, Error> {
    let config = Config::builder().version(IpVersion::V6).build();
    get_ip_with(config)
}

/// Get public IP address synchronously with a custom [`Config`].
///
/// # Errors
///
/// Returns an [`Error`] variant depending on the strategy and provider results.
pub fn get_ip_with(config: Config) -> Result<ProviderResult, Error> {
    let resolver = Resolver::new(config);
    resolver.resolve()
}
