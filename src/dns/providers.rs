//! Built-in DNS providers

use super::protocol::DnsClass;
use super::{DnsProvider, DnsRecordType};
use crate::provider::BoxedBlockingProvider;
#[cfg(feature = "tokio")]
use crate::provider::BoxedProvider;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

/// Google DNS o-o.myaddr
/// Query: o-o.myaddr.l.google.com TXT record via ns1.google.com
pub fn google() -> DnsProvider {
    DnsProvider::new(
        "Google DNS",
        "o-o.myaddr.l.google.com",
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(216, 239, 32, 10), 53)), // ns1.google.com
        DnsRecordType::Txt,
    )
    .with_v6_resolver(SocketAddr::V6(SocketAddrV6::new(
        Ipv6Addr::new(0x2001, 0x4860, 0x4802, 0x0032, 0, 0, 0, 0x000a),
        53,
        0,
        0,
    )))
}

/// Cloudflare whoami
/// Query: whoami.cloudflare TXT/CH via 1.1.1.1
/// Note: This uses DNS class CHAOS (CH), not IN
pub fn cloudflare() -> DnsProvider {
    DnsProvider::new(
        "Cloudflare DNS",
        "whoami.cloudflare",
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 1, 1, 1), 53)),
        DnsRecordType::Txt,
    )
    .with_class(DnsClass::Ch)
    .with_v6_resolver(SocketAddr::V6(SocketAddrV6::new(
        Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111),
        53,
        0,
        0,
    )))
}

/// OpenDNS myip service
/// Query: myip.opendns.com A record via resolver1.opendns.com
pub fn opendns() -> DnsProvider {
    DnsProvider::new(
        "OpenDNS",
        "myip.opendns.com",
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(208, 67, 222, 222), 53)),
        DnsRecordType::Address,
    )
}

/// List all available DNS provider names
pub fn provider_names() -> &'static [&'static str] {
    &["Google DNS", "Cloudflare DNS", "OpenDNS"]
}

/// Get default blocking DNS providers
pub fn default_blocking_providers() -> Vec<BoxedBlockingProvider> {
    vec![
        Box::new(cloudflare()),
        Box::new(google()),
        Box::new(opendns()),
    ]
}

/// Get default async DNS providers
#[cfg(feature = "tokio")]
pub fn default_providers() -> Vec<BoxedProvider> {
    vec![
        Box::new(cloudflare()),
        Box::new(google()),
        Box::new(opendns()),
    ]
}
