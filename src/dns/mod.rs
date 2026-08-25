//! DNS protocol implementation for public IP detection
//!
//! Uses DNS TXT/A records from special domains to detect public IP.
//! This implementation uses raw UDP sockets instead of external DNS libraries.
//!
//! # Security
//!
//! Transaction IDs are generated with [`getrandom`] (OS-level CSPRNG),
//! preventing DNS transaction ID spoofing attacks.

mod protocol;
pub(crate) mod providers;

pub use protocol::DnsClass;

#[cfg(feature = "tokio")]
pub use providers::default_providers;
pub use providers::{default_blocking_providers, provider_names};

use crate::error::ProviderError;
#[cfg(feature = "tokio")]
use crate::provider::Provider;
use crate::provider::{BlockingProvider, BoxedBlockingProvider};
use crate::types::{IpVersion, Protocol};
use protocol::{build_query, parse_response, RecordType};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

#[cfg(feature = "tokio")]
use std::future::Future;
#[cfg(feature = "tokio")]
use std::pin::Pin;

/// Record type for DNS query
#[derive(Debug, Clone, Copy)]
pub enum DnsRecordType {
    /// A/AAAA record (direct IP)
    Address,
    /// TXT record (IP as text)
    Txt,
}

/// DNS provider configuration
#[derive(Debug, Clone)]
pub struct DnsProvider {
    name: String,
    query_domain: String,
    resolver_addr: SocketAddr,
    resolver_addr_v6: Option<SocketAddr>,
    record_type: DnsRecordType,
    dns_class: DnsClass,
    supports_v4: bool,
    supports_v6: bool,
}

impl DnsProvider {
    /// Create a new DNS provider
    pub fn new(
        name: impl Into<String>,
        query_domain: impl Into<String>,
        resolver_addr: SocketAddr,
        record_type: DnsRecordType,
    ) -> Self {
        Self {
            name: name.into(),
            query_domain: query_domain.into(),
            resolver_addr,
            resolver_addr_v6: None,
            record_type,
            dns_class: DnsClass::In,
            supports_v4: true,
            supports_v6: false,
        }
    }

    /// Set DNS class (for special queries like Cloudflare CHAOS)
    pub fn with_class(mut self, class: DnsClass) -> Self {
        self.dns_class = class;
        self
    }

    /// Set IPv6 support
    pub fn with_v6(mut self, supports: bool) -> Self {
        self.supports_v6 = supports;
        self
    }

    /// Set IPv6 resolver address
    ///
    /// When requesting IPv6, the query is sent to this resolver so the
    /// DNS server sees the client's IPv6 source address.
    pub fn with_v6_resolver(mut self, addr: SocketAddr) -> Self {
        self.resolver_addr_v6 = Some(addr);
        self.supports_v6 = true;
        self
    }

    fn select_resolver(&self, version: IpVersion) -> SocketAddr {
        match version {
            IpVersion::V6 => self.resolver_addr_v6.unwrap_or(self.resolver_addr),
            _ => self.resolver_addr,
        }
    }

    fn select_record_type(&self, version: IpVersion) -> RecordType {
        match self.record_type {
            DnsRecordType::Address => match version {
                IpVersion::V6 => RecordType::Aaaa,
                _ => RecordType::A,
            },
            DnsRecordType::Txt => RecordType::Txt,
        }
    }

    fn extract_ip(
        &self,
        results: Vec<String>,
        version: IpVersion,
    ) -> Result<IpAddr, ProviderError> {
        for result in results {
            for part in result.split_whitespace() {
                let ip_str = part.split('/').next().unwrap_or(part);
                if let Ok(ip) = IpAddr::from_str(ip_str) {
                    match version {
                        IpVersion::V4 if ip.is_ipv4() => return Ok(ip),
                        IpVersion::V6 if ip.is_ipv6() => return Ok(ip),
                        IpVersion::Any => return Ok(ip),
                        _ => continue,
                    }
                }
            }
        }

        Err(ProviderError::message(
            &self.name,
            "no valid IP in DNS response",
        ))
    }

    fn validate_response(query: &[u8], response: &[u8]) -> Result<(), &'static str> {
        if query.len() < 2 || response.len() < 12 {
            return Err("response too short");
        }
        if response[..2] != query[..2] {
            return Err("transaction ID mismatch");
        }
        if response[2] & 0x80 == 0 {
            return Err("packet is not a DNS response");
        }
        if response[2] & 0x02 != 0 {
            return Err("truncated DNS response");
        }
        Ok(())
    }

    /// Query for IP address synchronously using standard UDP sockets with a timeout
    pub fn query_blocking(
        &self,
        version: IpVersion,
        timeout: Duration,
    ) -> Result<IpAddr, ProviderError> {
        if timeout.is_zero() {
            return Err(ProviderError::message(&self.name, "timeout"));
        }

        let resolver = self.select_resolver(version);
        let record_type = self.select_record_type(version);

        let query = build_query(&self.query_domain, record_type, self.dns_class)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let bind_addr = if resolver.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };
        let socket =
            std::net::UdpSocket::bind(bind_addr).map_err(|e| ProviderError::new(&self.name, e))?;

        socket
            .set_read_timeout(Some(timeout))
            .map_err(|e| ProviderError::new(&self.name, e))?;
        socket
            .set_write_timeout(Some(timeout))
            .map_err(|e| ProviderError::new(&self.name, e))?;

        socket
            .connect(resolver)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        socket
            .send(&query)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        let mut buf = [0u8; 1232]; // DNS Flag Day 2020 safe UDP size (RFC 6891 EDNS0)
        let len = socket
            .recv(&mut buf)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        Self::validate_response(&query, &buf[..len])
            .map_err(|e| ProviderError::message(&self.name, e))?;

        let results = parse_response(&buf[..len], record_type)
            .map_err(|e| ProviderError::message(&self.name, e))?;

        self.extract_ip(results, version)
    }

    /// Query for IP address using raw UDP asynchronously
    #[cfg(feature = "tokio")]
    async fn query(&self, version: IpVersion) -> Result<IpAddr, ProviderError> {
        let resolver = self.select_resolver(version);
        let record_type = self.select_record_type(version);

        // Build query packet
        let query = build_query(&self.query_domain, record_type, self.dns_class)
            .map_err(|e| ProviderError::new(&self.name, e))?;

        // Create UDP socket
        let bind_addr = if resolver.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };
        let socket = tokio::net::UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        socket
            .connect(resolver)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        // Send query
        socket
            .send(&query)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        // Receive response
        let mut buf = [0u8; 1232]; // DNS Flag Day 2020 safe UDP size (RFC 6891 EDNS0)
        let len = socket
            .recv(&mut buf)
            .await
            .map_err(|e| ProviderError::new(&self.name, e))?;

        Self::validate_response(&query, &buf[..len])
            .map_err(|e| ProviderError::message(&self.name, e))?;

        // Parse response
        let results = parse_response(&buf[..len], record_type)
            .map_err(|e| ProviderError::message(&self.name, e))?;

        self.extract_ip(results, version)
    }
}

impl BlockingProvider for DnsProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Dns
    }

    fn supports_v4(&self) -> bool {
        self.supports_v4
    }

    fn supports_v6(&self) -> bool {
        self.supports_v6
    }

    fn get_ip(&self, version: IpVersion, timeout: Duration) -> Result<IpAddr, ProviderError> {
        self.query_blocking(version, timeout)
    }

    fn clone_box(&self) -> BoxedBlockingProvider {
        Box::new(self.clone())
    }
}

#[cfg(feature = "tokio")]
impl Provider for DnsProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Dns
    }

    fn supports_v4(&self) -> bool {
        self.supports_v4
    }

    fn supports_v6(&self) -> bool {
        self.supports_v6
    }

    fn get_ip(
        &self,
        version: IpVersion,
    ) -> Pin<Box<dyn Future<Output = Result<IpAddr, ProviderError>> + Send + '_>> {
        Box::pin(self.query(version))
    }
}

#[cfg(test)]
mod blocking_tests {
    use super::{DnsProvider, DnsRecordType};
    use crate::types::IpVersion;
    use std::net::UdpSocket;
    use std::time::{Duration, Instant};

    fn response_for(query: &[u8], transaction_id: Option<[u8; 2]>) -> Vec<u8> {
        let mut response = query.to_vec();
        if let Some(id) = transaction_id {
            response[..2].copy_from_slice(&id);
        }
        response[2] = 0x81;
        response[3] = 0x80;
        response[6] = 0;
        response[7] = 1;
        response.extend_from_slice(&[
            0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x04, 203, 0, 113, 9,
        ]);
        response
    }

    fn provider_for(addr: std::net::SocketAddr) -> DnsProvider {
        DnsProvider::new("local-dns", "example.test", addr, DnsRecordType::Address)
    }

    #[test]
    fn zero_timeout_fails_before_waiting_for_dns_response() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        std::thread::spawn(move || {
            let mut query = [0u8; 512];
            if let Ok((len, peer)) = server.recv_from(&mut query) {
                std::thread::sleep(Duration::from_millis(100));
                let _ = server.send_to(&response_for(&query[..len], None), peer);
            }
        });

        let started = Instant::now();
        let result = provider_for(addr).query_blocking(IpVersion::V4, Duration::ZERO);

        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_millis(50));
    }

    #[test]
    fn rejects_dns_response_with_wrong_transaction_id() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut query = [0u8; 512];
            let (len, peer) = server.recv_from(&mut query).unwrap();
            let wrong_id = [query[0].wrapping_add(1), query[1]];
            let _ = server.send_to(&response_for(&query[..len], Some(wrong_id)), peer);
        });

        let result = provider_for(addr).query_blocking(IpVersion::V4, Duration::from_secs(1));

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("transaction ID mismatch"));
    }

    #[test]
    fn rejects_dns_response_from_unexpected_sender() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut query = [0u8; 512];
            let (len, peer) = server.recv_from(&mut query).unwrap();
            let unexpected_sender = UdpSocket::bind("127.0.0.1:0").unwrap();
            let _ = unexpected_sender.send_to(&response_for(&query[..len], None), peer);
        });

        let result = provider_for(addr).query_blocking(IpVersion::V4, Duration::from_millis(30));

        assert!(result.is_err());
    }
}
