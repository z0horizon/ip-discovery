//! STUN protocol implementation for public IP detection
//!
//! Implements a minimal RFC 5389 STUN client for detecting public IP addresses.
//!
//! # Limitations
//!
//! - **No retransmission**: RFC 5389 §7.2.1 recommends retransmitting requests
//!   with exponential backoff (RTO ≥ 500 ms). This implementation sends a single
//!   binding request; packet loss is handled by the resolver's fallback strategy.
//!
//! # Security
//!
//! Transaction IDs are generated with [`getrandom`] (OS-level CSPRNG),
//! per RFC 8489 requirements.

mod message;
pub(crate) mod providers;

#[cfg(feature = "tokio")]
pub use providers::default_providers;
pub use providers::{default_blocking_providers, provider_names};

use crate::error::ProviderError;
#[cfg(feature = "tokio")]
use crate::provider::Provider;
use crate::provider::{BlockingProvider, BoxedBlockingProvider};
use crate::types::{IpVersion, Protocol};
use message::{StunMessage, StunMethod};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

#[cfg(feature = "tokio")]
use std::future::Future;
#[cfg(feature = "tokio")]
use std::pin::Pin;

/// STUN provider for IP detection
#[derive(Debug, Clone)]
pub struct StunProvider {
    name: String,
    server: String,
    port: u16,
}

impl StunProvider {
    /// Create a new STUN provider
    pub fn new(name: impl Into<String>, server: impl Into<String>, port: u16) -> Self {
        Self {
            name: name.into(),
            server: server.into(),
            port,
        }
    }

    fn select_addr(addrs: &[SocketAddr], version: IpVersion) -> Option<SocketAddr> {
        match version {
            IpVersion::V4 => addrs.iter().copied().find(SocketAddr::is_ipv4),
            IpVersion::V6 => addrs.iter().copied().find(SocketAddr::is_ipv6),
            IpVersion::Any => addrs
                .iter()
                .copied()
                .find(SocketAddr::is_ipv4)
                .or_else(|| addrs.first().copied()),
        }
    }

    fn process_response(
        request: &StunMessage,
        buf: &[u8],
        name: &str,
    ) -> Result<IpAddr, ProviderError> {
        let response = StunMessage::decode(buf).map_err(|e| ProviderError::message(name, e))?;

        if response.transaction_id() != request.transaction_id() {
            return Err(ProviderError::message(name, "transaction ID mismatch"));
        }

        if response.method() != StunMethod::Response {
            return Err(ProviderError::message(
                name,
                "not a binding success response",
            ));
        }

        response
            .get_mapped_address()
            .ok_or_else(|| ProviderError::message(name, "no mapped address in response"))
    }

    /// Perform STUN binding request synchronously using standard UDP sockets with a timeout
    pub fn binding_request_blocking(
        &self,
        version: IpVersion,
        timeout: Duration,
    ) -> Result<IpAddr, ProviderError> {
        use std::net::ToSocketAddrs;

        if timeout.is_zero() {
            return Err(ProviderError::message(&self.name, "timeout"));
        }

        let server_addr = format!("{}:{}", self.server, self.port);
        let addrs: Vec<SocketAddr> = server_addr
            .to_socket_addrs()
            .map_err(|e| ProviderError::new(&self.name, e))?
            .collect();

        let addr = Self::select_addr(&addrs, version).ok_or_else(|| {
            ProviderError::message(&self.name, "no suitable address for IP version")
        })?;

        let local_addr = if addr.is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0u16; 8], 0))
        };

        let socket =
            std::net::UdpSocket::bind(local_addr).map_err(|e| ProviderError::new(&self.name, e))?;

        socket
            .set_read_timeout(Some(timeout))
            .map_err(|e| ProviderError::new(&self.name, e))?;
        socket
            .set_write_timeout(Some(timeout))
            .map_err(|e| ProviderError::new(&self.name, e))?;

        socket
            .connect(addr)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let request =
            StunMessage::new(StunMethod::Request).map_err(|e| ProviderError::new(&self.name, e))?;
        let request_bytes = request.encode();

        socket
            .send(&request_bytes)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let mut buf = [0u8; 576]; // Minimum MTU
        let len = socket
            .recv(&mut buf)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        Self::process_response(&request, &buf[..len], &self.name)
    }

    /// Perform STUN binding request asynchronously
    #[cfg(feature = "tokio")]
    async fn binding_request(&self, version: IpVersion) -> Result<IpAddr, ProviderError> {
        let server_addr = format!("{}:{}", self.server, self.port);
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&server_addr)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?
            .collect();

        let addr = Self::select_addr(&addrs, version).ok_or_else(|| {
            ProviderError::message(&self.name, "no suitable address for IP version")
        })?;

        let local_addr = if addr.is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0u16; 8], 0))
        };

        let socket = tokio::net::UdpSocket::bind(local_addr)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        socket
            .connect(addr)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let request =
            StunMessage::new(StunMethod::Request).map_err(|e| ProviderError::new(&self.name, e))?;
        let request_bytes = request.encode();

        socket
            .send(&request_bytes)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let mut buf = [0u8; 576]; // Minimum MTU
        let len = socket
            .recv(&mut buf)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        Self::process_response(&request, &buf[..len], &self.name)
    }
}

impl BlockingProvider for StunProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Stun
    }

    fn supports_v4(&self) -> bool {
        true
    }

    fn supports_v6(&self) -> bool {
        true
    }

    fn get_ip(&self, version: IpVersion, timeout: Duration) -> Result<IpAddr, ProviderError> {
        self.binding_request_blocking(version, timeout)
    }

    fn clone_box(&self) -> BoxedBlockingProvider {
        Box::new(self.clone())
    }
}

#[cfg(feature = "tokio")]
impl Provider for StunProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Stun
    }

    fn supports_v4(&self) -> bool {
        true
    }

    fn supports_v6(&self) -> bool {
        true
    }

    fn get_ip(
        &self,
        version: IpVersion,
    ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>> {
        Box::pin(self.binding_request(version))
    }
}

#[cfg(test)]
mod blocking_tests {
    use super::{StunMessage, StunMethod, StunProvider};
    use crate::types::IpVersion;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
    use std::time::{Duration, Instant};

    #[test]
    fn any_prefers_ipv4_address() {
        let v6: SocketAddr = "[::1]:3478".parse().unwrap();
        let v4: SocketAddr = "127.0.0.1:3478".parse().unwrap();

        assert_eq!(
            StunProvider::select_addr(&[v6, v4], IpVersion::Any),
            Some(v4)
        );
    }

    #[test]
    fn zero_timeout_fails_before_waiting_for_stun_response() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        std::thread::spawn(move || {
            let mut request = [0u8; 512];
            if let Ok((_, peer)) = server.recv_from(&mut request) {
                std::thread::sleep(Duration::from_millis(100));
                let _ = server.send_to(&[0u8; 20], peer);
            }
        });

        let provider = StunProvider::new("local-stun", "127.0.0.1", port);
        let started = Instant::now();
        let result = provider.binding_request_blocking(IpVersion::V4, Duration::ZERO);

        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_millis(50));
    }

    #[test]
    fn version_filter_selects_requested_family() {
        let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3478);
        let v6: SocketAddr = "[::1]:3478".parse().unwrap();

        assert_eq!(
            StunProvider::select_addr(&[v6, v4], IpVersion::V4),
            Some(v4)
        );
        assert_eq!(
            StunProvider::select_addr(&[v4, v6], IpVersion::V6),
            Some(v6)
        );
    }

    #[test]
    fn rejects_non_success_message_with_mapped_address() {
        let request = StunMessage::new(StunMethod::Request).unwrap();
        let mut response = request.encode();
        response[2..4].copy_from_slice(&12u16.to_be_bytes());
        response.extend_from_slice(&0x0001u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.extend_from_slice(&[0, 1, 0, 0, 203, 0, 113, 1]);

        let error = StunProvider::process_response(&request, &response, "test").unwrap_err();
        assert!(error.to_string().contains("not a binding success response"));

        response[0..2].copy_from_slice(&0x0111u16.to_be_bytes());
        let error = StunProvider::process_response(&request, &response, "test").unwrap_err();
        assert!(error.to_string().contains("not a binding success response"));
    }
}
