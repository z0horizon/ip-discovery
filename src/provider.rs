//! Provider traits and boxed type aliases.
//!
//! All IP detection backends (DNS, HTTP, STUN) implement [`BlockingProvider`]
//! for synchronous execution, and optionally [`Provider`] for async execution.
//! Custom providers can also be created by implementing these traits.

use crate::error::ProviderError;
use crate::types::{IpVersion, Protocol};
use std::net::IpAddr;
use std::time::Duration;

#[cfg(feature = "tokio")]
use std::future::Future;
#[cfg(feature = "tokio")]
use std::pin::Pin;

/// Trait for synchronous IP detection providers
pub trait BlockingProvider: Send + Sync {
    /// Provider name for identification
    fn name(&self) -> &str;

    /// Protocol used by this provider
    fn protocol(&self) -> Protocol;

    /// Whether this provider supports IPv4
    fn supports_v4(&self) -> bool {
        true
    }

    /// Whether this provider supports IPv6
    fn supports_v6(&self) -> bool {
        false
    }

    /// Check if provider supports the given IP version
    fn supports_version(&self, version: IpVersion) -> bool {
        match version {
            IpVersion::V4 => self.supports_v4(),
            IpVersion::V6 => self.supports_v6(),
            IpVersion::Any => self.supports_v4() || self.supports_v6(),
        }
    }

    /// Get the public IP address synchronously with the given timeout.
    ///
    /// Implementations should honor `timeout` for their own I/O. The blocking
    /// resolver also enforces the caller-visible deadline, but Rust cannot
    /// forcibly cancel an OS thread that is already executing provider code;
    /// work from a non-cooperative custom provider may continue in the
    /// background after the resolver returns a timeout.
    fn get_ip(&self, version: IpVersion, timeout: Duration) -> Result<IpAddr, ProviderError>;

    /// Clone this provider into a boxed trait object
    fn clone_box(&self) -> BoxedBlockingProvider;
}

/// Type-erased synchronous provider, used internally to store heterogeneous providers.
pub type BoxedBlockingProvider = Box<dyn BlockingProvider>;

impl Clone for BoxedBlockingProvider {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Trait for asynchronous IP detection providers
#[cfg(feature = "tokio")]
pub trait Provider: Send + Sync {
    /// Provider name for identification
    fn name(&self) -> &str;

    /// Protocol used by this provider
    fn protocol(&self) -> Protocol;

    /// Whether this provider supports IPv4
    fn supports_v4(&self) -> bool {
        true
    }

    /// Whether this provider supports IPv6
    fn supports_v6(&self) -> bool {
        false
    }

    /// Check if provider supports the given IP version
    fn supports_version(&self, version: IpVersion) -> bool {
        match version {
            IpVersion::V4 => self.supports_v4(),
            IpVersion::V6 => self.supports_v6(),
            IpVersion::Any => self.supports_v4() || self.supports_v6(),
        }
    }

    /// Get the public IP address asynchronously
    fn get_ip(
        &self,
        version: IpVersion,
    ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>>;
}

/// Type-erased asynchronous provider, used internally to store heterogeneous providers.
#[cfg(feature = "tokio")]
pub type BoxedProvider = Box<dyn Provider>;

/// Stub provider returned when a protocol feature (dns/http/stun) is not enabled.
/// Always returns an error explaining which feature is missing.
#[derive(Clone)]
pub(crate) struct DisabledProvider(pub(crate) String);

impl BlockingProvider for DisabledProvider {
    fn name(&self) -> &str {
        &self.0
    }

    fn protocol(&self) -> Protocol {
        Protocol::Http // doesn't matter, will always error
    }

    fn get_ip(&self, _version: IpVersion, _timeout: Duration) -> Result<IpAddr, ProviderError> {
        Err(ProviderError::message(
            &self.0,
            "provider feature not enabled",
        ))
    }

    fn clone_box(&self) -> BoxedBlockingProvider {
        Box::new(self.clone())
    }
}

#[cfg(feature = "tokio")]
impl Provider for DisabledProvider {
    fn name(&self) -> &str {
        &self.0
    }

    fn protocol(&self) -> Protocol {
        Protocol::Http // doesn't matter, will always error
    }

    fn get_ip(
        &self,
        _version: IpVersion,
    ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>> {
        let name = self.0.clone();
        Box::pin(async move {
            Err(ProviderError::message(
                &name,
                "provider feature not enabled",
            ))
        })
    }
}
