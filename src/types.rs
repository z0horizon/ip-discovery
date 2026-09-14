//! Core types for ip-discovery

use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

/// Protocol used to detect public IP
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Protocol {
    /// DNS-based detection (e.g., OpenDNS, Cloudflare DNS)
    Dns,
    /// HTTP/HTTPS-based detection
    Http,
    /// STUN protocol (RFC 5389)
    Stun,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Dns => write!(f, "DNS"),
            Protocol::Http => write!(f, "HTTP"),
            Protocol::Stun => write!(f, "STUN"),
        }
    }
}

/// Error returned when parsing a [`Protocol`] from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseProtocolError(pub(crate) String);

impl std::fmt::Display for ParseProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid protocol '{}', expected 'dns', 'stun', or 'http'",
            self.0
        )
    }
}

impl std::error::Error for ParseProtocolError {}

impl FromStr for Protocol {
    type Err = ParseProtocolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "dns" => Ok(Self::Dns),
            "stun" => Ok(Self::Stun),
            "http" | "https" => Ok(Self::Http),
            _ => Err(ParseProtocolError(s.to_string())),
        }
    }
}

/// IP version preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum IpVersion {
    /// IPv4 only
    V4,
    /// IPv6 only
    V6,
    /// Any IP version (prefer IPv4)
    #[default]
    Any,
}

impl IpVersion {
    pub(crate) fn matches(self, ip: IpAddr) -> bool {
        match self {
            Self::V4 => ip.is_ipv4(),
            Self::V6 => ip.is_ipv6(),
            Self::Any => true,
        }
    }
}

/// Error returned when parsing an [`IpVersion`] from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIpVersionError(pub(crate) String);

impl std::fmt::Display for ParseIpVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid IP version '{}', expected 'v4', 'v6', or 'any'",
            self.0
        )
    }
}

impl std::error::Error for ParseIpVersionError {}

impl FromStr for IpVersion {
    type Err = ParseIpVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "v4" | "4" | "ipv4" => Ok(Self::V4),
            "v6" | "6" | "ipv6" => Ok(Self::V6),
            "any" => Ok(Self::Any),
            _ => Err(ParseIpVersionError(s.to_string())),
        }
    }
}

/// Result from a successful IP lookup
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResult {
    /// The detected public IP address
    pub ip: IpAddr,
    /// Name of the provider that returned this result
    pub provider: String,
    /// Protocol used for detection
    pub protocol: Protocol,
    /// Time taken to get the result
    pub latency: Duration,
}

impl ProviderResult {
    /// Extract the IPv4 address from the result, if present.
    pub fn ipv4(&self) -> Option<std::net::Ipv4Addr> {
        match self.ip {
            IpAddr::V4(v4) => Some(v4),
            _ => None,
        }
    }

    /// Extract the IPv6 address from the result, if present.
    pub fn ipv6(&self) -> Option<std::net::Ipv6Addr> {
        match self.ip {
            IpAddr::V6(v6) => Some(v6),
            _ => None,
        }
    }

    /// Returns `true` if the detected public IP is an IPv4 address.
    #[inline]
    pub fn is_ipv4(&self) -> bool {
        self.ip.is_ipv4()
    }

    /// Returns `true` if the detected public IP is an IPv6 address.
    #[inline]
    pub fn is_ipv6(&self) -> bool {
        self.ip.is_ipv6()
    }
}

impl std::fmt::Display for ProviderResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (via {} over {}, {:?})",
            self.ip, self.provider, self.protocol, self.latency
        )
    }
}

/// Built-in IP detection providers
///
/// Each variant represents a specific provider service.
/// Use with [`Config::builder()`](crate::Config::builder) to select which providers to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BuiltinProvider {
    // --- STUN providers ---
    /// Google STUN server (stun.l.google.com)
    GoogleStun,
    /// Google STUN server 1 (stun1.l.google.com)
    GoogleStun1,
    /// Google STUN server 2 (stun2.l.google.com)
    GoogleStun2,
    /// Cloudflare STUN server (stun.cloudflare.com)
    CloudflareStun,

    // --- DNS providers ---
    /// Google DNS via o-o.myaddr.l.google.com TXT
    GoogleDns,
    /// Cloudflare DNS via whoami.cloudflare TXT/CH
    CloudflareDns,
    /// OpenDNS via myip.opendns.com
    OpenDns,

    // --- HTTP providers ---
    /// Cloudflare 1.1.1.1/cdn-cgi/trace
    CloudflareHttp,
    /// AWS checkip.amazonaws.com
    Aws,
}

impl BuiltinProvider {
    /// Get the protocol this provider uses
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::GoogleStun | Self::GoogleStun1 | Self::GoogleStun2 | Self::CloudflareStun => {
                Protocol::Stun
            }
            Self::GoogleDns | Self::CloudflareDns | Self::OpenDns => Protocol::Dns,
            Self::CloudflareHttp | Self::Aws => Protocol::Http,
        }
    }

    /// All available built-in providers, ordered by expected performance.
    ///
    /// Providers with both IPv4 and IPv6 support are listed first, followed by
    /// IPv4-only providers. Within each tier, UDP-based protocols (STUN, DNS)
    /// are preferred over HTTP due to lower overhead (no TLS handshake).
    ///
    /// This order is used by [`Strategy::First`](crate::Strategy::First).
    /// Run the benchmark example to find the optimal order for your network:
    /// `cargo run --example benchmark --all-features`
    pub const ALL: &'static [BuiltinProvider] = &[
        // Tier 1: UDP-based, IPv4 + IPv6
        Self::CloudflareStun,
        Self::CloudflareDns,
        Self::GoogleStun,
        Self::GoogleStun1,
        Self::GoogleStun2,
        Self::GoogleDns,
        // Tier 2: IPv4-only (fallback)
        Self::OpenDns,
        Self::CloudflareHttp,
        Self::Aws,
    ];

    /// Create the boxed blocking provider instance
    pub(crate) fn to_boxed_blocking(self) -> crate::provider::BoxedBlockingProvider {
        match self {
            #[cfg(feature = "stun")]
            Self::GoogleStun => Box::new(crate::stun::providers::google()),
            #[cfg(feature = "stun")]
            Self::GoogleStun1 => Box::new(crate::stun::providers::google1()),
            #[cfg(feature = "stun")]
            Self::GoogleStun2 => Box::new(crate::stun::providers::google2()),
            #[cfg(feature = "stun")]
            Self::CloudflareStun => Box::new(crate::stun::providers::cloudflare()),

            #[cfg(feature = "dns")]
            Self::GoogleDns => Box::new(crate::dns::providers::google()),
            #[cfg(feature = "dns")]
            Self::CloudflareDns => Box::new(crate::dns::providers::cloudflare()),
            #[cfg(feature = "dns")]
            Self::OpenDns => Box::new(crate::dns::providers::opendns()),

            #[cfg(feature = "http")]
            Self::CloudflareHttp => Box::new(crate::http::providers::cloudflare()),
            #[cfg(feature = "http")]
            Self::Aws => Box::new(crate::http::providers::aws()),

            // Feature not enabled — create a stub that always errors
            #[allow(unreachable_patterns)]
            other => Box::new(super::provider::DisabledProvider(format!("{:?}", other))),
        }
    }

    /// Create the boxed async provider instance
    #[cfg(feature = "tokio")]
    pub(crate) fn to_boxed(self) -> crate::provider::BoxedProvider {
        match self {
            #[cfg(feature = "stun")]
            Self::GoogleStun => Box::new(crate::stun::providers::google()),
            #[cfg(feature = "stun")]
            Self::GoogleStun1 => Box::new(crate::stun::providers::google1()),
            #[cfg(feature = "stun")]
            Self::GoogleStun2 => Box::new(crate::stun::providers::google2()),
            #[cfg(feature = "stun")]
            Self::CloudflareStun => Box::new(crate::stun::providers::cloudflare()),

            #[cfg(feature = "dns")]
            Self::GoogleDns => Box::new(crate::dns::providers::google()),
            #[cfg(feature = "dns")]
            Self::CloudflareDns => Box::new(crate::dns::providers::cloudflare()),
            #[cfg(feature = "dns")]
            Self::OpenDns => Box::new(crate::dns::providers::opendns()),

            #[cfg(feature = "http")]
            Self::CloudflareHttp => Box::new(crate::http::providers::cloudflare()),
            #[cfg(feature = "http")]
            Self::Aws => Box::new(crate::http::providers::aws()),

            // Feature not enabled — create a stub that always errors
            #[allow(unreachable_patterns)]
            other => Box::new(super::provider::DisabledProvider(format!("{:?}", other))),
        }
    }
}

impl std::fmt::Display for BuiltinProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GoogleStun => write!(f, "Google STUN"),
            Self::GoogleStun1 => write!(f, "Google STUN 1"),
            Self::GoogleStun2 => write!(f, "Google STUN 2"),
            Self::CloudflareStun => write!(f, "Cloudflare STUN"),
            Self::GoogleDns => write!(f, "Google DNS"),
            Self::CloudflareDns => write!(f, "Cloudflare DNS"),
            Self::OpenDns => write!(f, "OpenDNS"),
            Self::CloudflareHttp => write!(f, "Cloudflare"),
            Self::Aws => write!(f, "AWS"),
        }
    }
}

/// Error returned when parsing a [`BuiltinProvider`] from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseProviderError(pub(crate) String);

impl std::fmt::Display for ParseProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid provider '{}'", self.0)
    }
}

impl std::error::Error for ParseProviderError {}

impl FromStr for BuiltinProvider {
    type Err = ParseProviderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "google-stun" => Ok(Self::GoogleStun),
            "google-stun1" => Ok(Self::GoogleStun1),
            "google-stun2" => Ok(Self::GoogleStun2),
            "cloudflare-stun" => Ok(Self::CloudflareStun),
            "google-dns" => Ok(Self::GoogleDns),
            "cloudflare-dns" => Ok(Self::CloudflareDns),
            "opendns" | "open-dns" => Ok(Self::OpenDns),
            "cloudflare-http" => Ok(Self::CloudflareHttp),
            "aws" => Ok(Self::Aws),
            _ => Err(ParseProviderError(s.to_string())),
        }
    }
}
