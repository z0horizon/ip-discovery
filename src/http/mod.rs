//! HTTP/HTTPS protocol implementation for public IP detection
//!
//! Uses various HTTP-based IP detection services.

pub(crate) mod providers;

#[cfg(feature = "tokio")]
pub use providers::default_providers;
pub use providers::{default_blocking_providers, provider_names};

use crate::error::ProviderError;
#[cfg(feature = "tokio")]
use crate::provider::Provider;
use crate::provider::{BlockingProvider, BoxedBlockingProvider};
use crate::types::{IpVersion, Protocol};
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

#[cfg(feature = "tokio")]
use std::future::Future;
#[cfg(feature = "tokio")]
use std::pin::Pin;

/// Response parser function type
pub type ResponseParser = fn(&str) -> Option<IpAddr>;

/// Parse plain text IP response
pub fn parse_plain_text(text: &str) -> Option<IpAddr> {
    IpAddr::from_str(text.trim()).ok()
}

/// Parse Cloudflare trace response (key=value format)
pub fn parse_cloudflare_trace(text: &str) -> Option<IpAddr> {
    for line in text.lines() {
        if let Some(ip_str) = line.strip_prefix("ip=") {
            return IpAddr::from_str(ip_str.trim()).ok();
        }
    }
    None
}

/// HTTP provider configuration
#[derive(Clone)]
pub struct HttpProvider {
    name: String,
    url_v4: Option<String>,
    url_v6: Option<String>,
    parser: ResponseParser,
    #[cfg(feature = "tokio")]
    client: reqwest::Client,
}

impl HttpProvider {
    /// Create a new HTTP provider (plain text response)
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        #[cfg(feature = "tokio")]
        let client = reqwest::Client::builder()
            .user_agent(concat!("ip-discovery/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();

        Self {
            name: name.into(),
            url_v4: Some(url.into()),
            url_v6: None,
            parser: parse_plain_text,
            #[cfg(feature = "tokio")]
            client,
        }
    }

    /// Set custom response parser
    pub fn with_parser(mut self, parser: ResponseParser) -> Self {
        self.parser = parser;
        self
    }

    /// Set IPv6 URL
    pub fn with_v6_url(mut self, url: impl Into<String>) -> Self {
        self.url_v6 = Some(url.into());
        self
    }

    /// Get URL for IP version
    fn get_url(&self, version: IpVersion) -> Option<&str> {
        match version {
            IpVersion::V6 => self.url_v6.as_deref().or(self.url_v4.as_deref()),
            _ => self.url_v4.as_deref(),
        }
    }

    /// Fetch IP from URL synchronously using reqwest blocking client with a timeout
    pub fn fetch_blocking(
        &self,
        version: IpVersion,
        timeout: Duration,
    ) -> Result<IpAddr, ProviderError> {
        let url = self
            .get_url(version)
            .ok_or_else(|| ProviderError::message(&self.name, "no URL for IP version"))?;

        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("ip-discovery/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();

        let response = client
            .get(url)
            .send()
            .map_err(|e| ProviderError::new(&self.name, e))?;

        if !response.status().is_success() {
            return Err(ProviderError::message(
                &self.name,
                format!("HTTP error: {}", response.status()),
            ));
        }

        let text = response
            .text()
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let ip = (self.parser)(&text)
            .ok_or_else(|| ProviderError::message(&self.name, "failed to parse response"))?;
        if version.matches(ip) {
            Ok(ip)
        } else {
            Err(ProviderError::message(
                &self.name,
                "provider returned unexpected IP version",
            ))
        }
    }

    /// Fetch IP from URL asynchronously
    #[cfg(feature = "tokio")]
    async fn fetch(&self, version: IpVersion) -> Result<IpAddr, ProviderError> {
        let url = self
            .get_url(version)
            .ok_or_else(|| ProviderError::message(&self.name, "no URL for IP version"))?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        if !response.status().is_success() {
            return Err(ProviderError::message(
                &self.name,
                format!("HTTP error: {}", response.status()),
            ));
        }

        let text = response
            .text()
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let ip = (self.parser)(&text)
            .ok_or_else(|| ProviderError::message(&self.name, "failed to parse response"))?;
        if version.matches(ip) {
            Ok(ip)
        } else {
            Err(ProviderError::message(
                &self.name,
                "provider returned unexpected IP version",
            ))
        }
    }
}

impl std::fmt::Debug for HttpProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpProvider")
            .field("name", &self.name)
            .field("url_v4", &self.url_v4)
            .field("url_v6", &self.url_v6)
            .finish()
    }
}

impl BlockingProvider for HttpProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Http
    }

    fn supports_v4(&self) -> bool {
        self.url_v4.is_some()
    }

    fn supports_v6(&self) -> bool {
        self.url_v6.is_some()
    }

    fn get_ip(&self, version: IpVersion, timeout: Duration) -> Result<IpAddr, ProviderError> {
        self.fetch_blocking(version, timeout)
    }

    fn clone_box(&self) -> BoxedBlockingProvider {
        Box::new(self.clone())
    }
}

#[cfg(feature = "tokio")]
impl Provider for HttpProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Http
    }

    fn supports_v4(&self) -> bool {
        self.url_v4.is_some()
    }

    fn supports_v6(&self) -> bool {
        self.url_v6.is_some()
    }

    fn get_ip(
        &self,
        version: IpVersion,
    ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>> {
        Box::pin(self.fetch(version))
    }
}
