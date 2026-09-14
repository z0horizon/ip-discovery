//! # ip-discovery
//!
//! A lightweight, high-performance Rust library for detecting public IP addresses
//! via DNS, HTTP, and STUN protocols with fallback support.
//!
//! ## Features
//!
//! - **Multi-protocol support**: DNS, HTTP/HTTPS, STUN (RFC 5389)
//! - **Trusted providers**: Google, Cloudflare, AWS, OpenDNS
//! - **Fallback mechanism**: Automatic retry with different providers
//! - **Flexible strategies**: First success, race (fastest), or consensus
//! - **Zero-dependency protocols**: DNS and STUN use raw UDP sockets
//! - **Synchronous & Asynchronous**: Zero-dependency blocking API by default, or Tokio async via feature
//!
//! ## Synchronous (Blocking) Quick Start
//!
//! ```rust,no_run
//! use ip_discovery::blocking::{get_ip, get_ipv4};
//!
//! // Get any IP address (IPv4 or IPv6) synchronously without Tokio runtime
//! if let Ok(result) = get_ip() {
//!     println!("Public IP: {} (via {})", result.ip, result.provider);
//! }
//!
//! // Get IPv4 specifically
//! if let Ok(result) = get_ipv4() {
//!     println!("IPv4: {}", result.ip);
//! }
//! ```
//!
//! ## Asynchronous Quick Start (with `tokio` feature)
//!
//! ```rust,no_run
//! # #[cfg(feature = "tokio")]
//! # #[tokio::main]
//! # async fn main() {
//! use ip_discovery::{get_ip, get_ipv4};
//!
//! // Get any IP address asynchronously
//! if let Ok(result) = get_ip().await {
//!     println!("Public IP: {} (via {})", result.ip, result.provider);
//! }
//! # }
//! # #[cfg(not(feature = "tokio"))]
//! # fn main() {}
//! ```

#![warn(missing_docs)]

pub mod blocking;
mod config;
mod error;
mod provider;
#[cfg(feature = "tokio")]
mod resolver;
mod types;

#[cfg(feature = "dns")]
pub mod dns;

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "stun")]
pub mod stun;

pub use config::{Config, ConfigBuilder, Strategy};
pub use error::{Error, ProviderError};
pub use provider::{BlockingProvider, BoxedBlockingProvider};
#[cfg(feature = "tokio")]
pub use provider::{BoxedProvider, Provider};
#[cfg(feature = "tokio")]
pub use resolver::Resolver;
pub use types::{
    BuiltinProvider, IpVersion, ParseIpVersionError, ParseProtocolError, ParseProviderError,
    Protocol, ProviderResult,
};

#[cfg(feature = "tokio")]
/// Get public IP address using default configuration asynchronously.
///
/// Uses all available protocols with the [`Strategy::First`] fallback strategy
/// and a 10-second per-provider timeout.
///
/// # Errors
///
/// Returns [`Error::AllProvidersFailed`] if every provider fails.
pub async fn get_ip() -> Result<ProviderResult, Error> {
    let config = Config::default();
    get_ip_with(config).await
}

#[cfg(feature = "tokio")]
/// Get public IPv4 address using default configuration asynchronously.
///
/// # Errors
///
/// Returns [`Error::AllProvidersFailed`] if no provider returns an IPv4 address.
pub async fn get_ipv4() -> Result<ProviderResult, Error> {
    let config = Config::builder().version(IpVersion::V4).build();
    get_ip_with(config).await
}

#[cfg(feature = "tokio")]
/// Get public IPv6 address using default configuration asynchronously.
///
/// # Errors
///
/// Returns [`Error::NoProvidersForVersion`] if no provider supports IPv6.
pub async fn get_ipv6() -> Result<ProviderResult, Error> {
    let config = Config::builder().version(IpVersion::V6).build();
    get_ip_with(config).await
}

#[cfg(feature = "tokio")]
/// Get public IP address with a custom [`Config`] asynchronously.
///
/// # Errors
///
/// Returns an [`Error`] variant depending on the strategy and provider results.
pub async fn get_ip_with(config: Config) -> Result<ProviderResult, Error> {
    let resolver = Resolver::new(config);
    resolver.resolve().await
}

/// Get the primary local private IPv4 address.
///
/// This function queries the OS routing table by creating a connectionless
/// UDP socket and connecting it to a public destination. No network packets are sent.
pub fn get_private_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // Using Google DNS to trigger routing table lookup for outbound IPv4 traffic.
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}

/// Get the primary local private IPv6 address.
///
/// This function queries the OS routing table by creating a connectionless
/// UDP socket and connecting it to a public destination. No network packets are sent.
pub fn get_private_ipv6() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("[::]:0").ok()?;
    // Using Google DNS IPv6 to trigger routing table lookup for outbound IPv6 traffic.
    socket.connect("[2001:4860:4860::8888]:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}
