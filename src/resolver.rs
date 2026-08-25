//! Resolution engine that coordinates providers and applies strategies.
//!
//! This module is the core orchestrator: it takes a [`Config`](crate::Config),
//! queries providers according to the chosen [`Strategy`](crate::Strategy),
//! and returns a [`ProviderResult`].
//!
//! The [`select_first`] and [`join_all_vec`] helper functions replace
//! `futures::select_all` / `futures::join_all` to avoid pulling in the
//! `futures-util` crate as a dependency.

use crate::config::{Config, Strategy};
use crate::error::{Error, ProviderError};
use crate::provider::BoxedProvider;
use crate::types::ProviderResult;
use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;
use tokio::time::timeout;

/// Boxed future returning a fallible provider result.
type BoxFut<'a> = Pin<Box<dyn Future<Output = Result<ProviderResult, ProviderError>> + Send + 'a>>;

/// Coordinates IP detection across configured providers.
///
/// Created via [`Resolver::new()`] with a [`Config`](crate::Config).
/// Call [`resolve()`](Resolver::resolve) to perform the lookup.
pub struct Resolver {
    config: Config,
}

impl Resolver {
    /// Create a new resolver with the given configuration
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Return an iterator over providers that support the configured IP version.
    #[inline]
    fn matching_providers(&self) -> impl Iterator<Item = &BoxedProvider> {
        self.config
            .providers
            .iter()
            .filter(|p| p.supports_version(self.config.version))
    }

    /// Wrap a single provider call in a timeout, returning a boxed future that
    /// produces either a [`ProviderResult`] or a [`ProviderError`].
    fn make_provider_future<'a>(&'a self, provider: &'a BoxedProvider) -> BoxFut<'a> {
        let provider_name = provider.name().to_string();
        let protocol = provider.protocol();
        let start = Instant::now();
        let fut = provider.get_ip(self.config.version);
        let timeout_duration = self.config.timeout;
        let version = self.config.version;

        Box::pin(async move {
            match timeout(timeout_duration, fut).await {
                Ok(Ok(ip)) => {
                    if !version.matches(ip) {
                        return Err(ProviderError::message(
                            provider_name,
                            "provider returned unexpected IP version",
                        ));
                    }
                    let latency = start.elapsed();
                    Ok(ProviderResult {
                        ip,
                        provider: provider_name,
                        protocol,
                        latency,
                    })
                }
                Ok(Err(e)) => Err(e),
                Err(_) => Err(ProviderError::message(provider_name, "timeout")),
            }
        })
    }

    /// Resolve the public IP address using the configured strategy.
    ///
    /// # Errors
    ///
    /// - [`Error::NoProvidersForVersion`] — no provider supports the requested IP version.
    /// - [`Error::AllProvidersFailed`] — every provider either failed or timed out.
    /// - [`Error::ConsensusNotReached`] — (consensus strategy) too few providers agreed.
    pub async fn resolve(&self) -> Result<ProviderResult, Error> {
        if self.matching_providers().next().is_none() {
            return Err(Error::NoProvidersForVersion);
        }

        match self.config.strategy {
            Strategy::First => self.resolve_first().await,
            Strategy::Race => self.resolve_race().await,
            Strategy::Consensus { min_agree } => self.resolve_consensus(min_agree).await,
        }
    }

    /// Try providers in order, return first success.
    async fn resolve_first(&self) -> Result<ProviderResult, Error> {
        let mut errors = Vec::new();

        for provider in self.matching_providers() {
            match self.make_provider_future(provider).await {
                Ok(result) => return Ok(result),
                Err(e) => errors.push(e),
            }
        }

        Err(Error::AllProvidersFailed(errors))
    }

    /// Race all providers concurrently, return fastest success.
    async fn resolve_race(&self) -> Result<ProviderResult, Error> {
        let mut futures: Vec<BoxFut<'_>> = self
            .matching_providers()
            .map(|p| self.make_provider_future(p))
            .collect();

        // Defensive: matching_providers() was already checked in resolve(),
        // but guard against direct calls to this method.
        if futures.is_empty() {
            return Err(Error::NoProvidersForVersion);
        }

        let mut errors = Vec::new();

        while !futures.is_empty() {
            let (result, _index, remaining) = select_first(futures).await;
            futures = remaining;

            match result {
                Ok(provider_result) => return Ok(provider_result),
                Err(e) => errors.push(e),
            }
        }

        Err(Error::AllProvidersFailed(errors))
    }

    /// Query all providers and require consensus.
    async fn resolve_consensus(&self, min_agree: usize) -> Result<ProviderResult, Error> {
        let futures: Vec<BoxFut<'_>> = self
            .matching_providers()
            .map(|p| self.make_provider_future(p))
            .collect();

        if futures.is_empty() {
            return Err(Error::NoProvidersForVersion);
        }

        let all_results = join_all_vec(futures).await;

        let mut ip_results: HashMap<IpAddr, Vec<ProviderResult>> = HashMap::new();
        let mut errors = Vec::new();

        for result in all_results {
            match result {
                Ok(pr) => ip_results.entry(pr.ip).or_default().push(pr),
                Err(e) => errors.push(e),
            }
        }

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

/// Select the first future to complete from a vec, returning the result,
/// the index in the **original** vec, and the remaining futures.
///
/// Note: `remaining` is **unordered** — `swap_remove` is used internally,
/// so the positions no longer correspond to the original input order.
///
/// Equivalent to `futures::select_all`, inlined to avoid the dependency.
///
/// # Polling safety
///
/// All futures are `Pin<Box<...>>` (i.e. `Unpin`), so `Pin::new(fut).poll(cx)`
/// is sound. Waker registration is delegated to each sub-future's poll impl;
/// when any sub-future's I/O becomes ready the shared waker is notified,
/// causing the entire `poll_fn` closure to be re-polled.
async fn select_first<F: Future + Unpin>(mut futures: Vec<F>) -> (F::Output, usize, Vec<F>) {
    std::future::poll_fn(|cx: &mut Context<'_>| {
        for (i, fut) in futures.iter_mut().enumerate() {
            if let Poll::Ready(output) = Pin::new(fut).poll(cx) {
                futures.swap_remove(i);
                return Poll::Ready((output, i, std::mem::take(&mut futures)));
            }
        }
        Poll::Pending
    })
    .await
}

/// Join all futures in a vec, returning a vec of results in the original order.
///
/// Equivalent to `futures::join_all`, inlined to avoid the dependency.
///
/// # Polling safety
///
/// Same as [`select_first`]. The `is_some()` guard ensures each future is
/// polled only while still pending, and `done` is never double-counted.
async fn join_all_vec<T, F: Future<Output = T> + Unpin>(mut futures: Vec<F>) -> Vec<T> {
    let total = futures.len();
    let mut results: Vec<Option<T>> = (0..total).map(|_| None).collect();
    let mut done = 0;

    std::future::poll_fn(|cx: &mut Context<'_>| {
        for (i, fut) in futures.iter_mut().enumerate() {
            if results[i].is_some() {
                continue;
            }
            if let Poll::Ready(output) = Pin::new(fut).poll(cx) {
                results[i] = Some(output);
                done += 1;
            }
        }
        if done == total {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;

    results
        .into_iter()
        .map(|r| r.expect("bug: future completed but result slot is empty"))
        .collect()
}
