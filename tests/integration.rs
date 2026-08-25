//! Integration tests - require network access
//!
//! Run with: `cargo test --test integration -- --ignored`

use ip_discovery::{Config, Protocol, Strategy};
use std::time::Duration;

#[cfg(feature = "tokio")]
use ip_discovery::{get_ip, get_ip_with, get_ipv4};

#[cfg(feature = "tokio")]
#[tokio::test]
#[ignore = "requires network"]
async fn test_get_ip_default() {
    let result = get_ip().await;
    assert!(result.is_ok(), "get_ip() failed: {:?}", result.err());
    let result = result.unwrap();
    assert!(!result.ip.is_loopback());
    assert!(!result.ip.is_unspecified());
    assert!(!result.provider.is_empty());
}

#[cfg(feature = "tokio")]
#[tokio::test]
#[ignore = "requires network"]
async fn test_get_ipv4() {
    let result = get_ipv4().await;
    assert!(result.is_ok(), "get_ipv4() failed: {:?}", result.err());
    assert!(result.unwrap().ip.is_ipv4());
}

#[cfg(feature = "tokio")]
#[tokio::test]
#[ignore = "requires network"]
async fn test_stun_only() {
    let config = Config::builder()
        .protocols(&[Protocol::Stun])
        .timeout(Duration::from_secs(5))
        .build();
    let result = get_ip_with(config).await;
    assert!(result.is_ok(), "STUN failed: {:?}", result.err());
}

#[cfg(feature = "tokio")]
#[tokio::test]
#[ignore = "requires network"]
async fn test_dns_only() {
    let config = Config::builder()
        .protocols(&[Protocol::Dns])
        .timeout(Duration::from_secs(5))
        .build();
    let result = get_ip_with(config).await;
    assert!(result.is_ok(), "DNS failed: {:?}", result.err());
}

#[cfg(feature = "tokio")]
#[tokio::test]
#[ignore = "requires network"]
async fn test_http_only() {
    let config = Config::builder()
        .protocols(&[Protocol::Http])
        .timeout(Duration::from_secs(10))
        .build();
    let result = get_ip_with(config).await;
    assert!(result.is_ok(), "HTTP failed: {:?}", result.err());
}

#[cfg(feature = "tokio")]
#[tokio::test]
#[ignore = "requires network"]
async fn test_race_strategy() {
    let config = Config::builder()
        .strategy(Strategy::Race)
        .timeout(Duration::from_secs(10))
        .build();
    let result = get_ip_with(config).await;
    assert!(result.is_ok(), "Race failed: {:?}", result.err());
}

#[cfg(feature = "tokio")]
#[tokio::test]
#[ignore = "requires network"]
async fn test_consensus_strategy() {
    let config = Config::builder()
        .strategy(Strategy::Consensus { min_agree: 2 })
        .timeout(Duration::from_secs(10))
        .build();
    let result = get_ip_with(config).await;
    assert!(result.is_ok(), "Consensus failed: {:?}", result.err());
}

// ── Synchronous Blocking Integration Tests ──────────────────────────────

#[test]
#[ignore = "requires network"]
fn test_blocking_get_ip_default() {
    let result = ip_discovery::blocking::get_ip();
    assert!(
        result.is_ok(),
        "blocking::get_ip() failed: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert!(!result.ip.is_loopback());
    assert!(!result.ip.is_unspecified());
    assert!(!result.provider.is_empty());
}

#[test]
#[ignore = "requires network"]
fn test_blocking_get_ipv4() {
    let result = ip_discovery::blocking::get_ipv4();
    assert!(
        result.is_ok(),
        "blocking::get_ipv4() failed: {:?}",
        result.err()
    );
    assert!(result.unwrap().ip.is_ipv4());
}

#[test]
#[ignore = "requires network"]
fn test_blocking_stun_only() {
    let config = Config::builder()
        .protocols(&[Protocol::Stun])
        .timeout(Duration::from_secs(5))
        .build();
    let result = ip_discovery::blocking::get_ip_with(config);
    assert!(result.is_ok(), "blocking STUN failed: {:?}", result.err());
}

#[test]
#[ignore = "requires network"]
fn test_blocking_dns_only() {
    let config = Config::builder()
        .protocols(&[Protocol::Dns])
        .timeout(Duration::from_secs(5))
        .build();
    let result = ip_discovery::blocking::get_ip_with(config);
    assert!(result.is_ok(), "blocking DNS failed: {:?}", result.err());
}

#[test]
#[ignore = "requires network"]
fn test_blocking_race_strategy() {
    let config = Config::builder()
        .strategy(Strategy::Race)
        .timeout(Duration::from_secs(10))
        .build();
    let result = ip_discovery::blocking::get_ip_with(config);
    assert!(result.is_ok(), "blocking Race failed: {:?}", result.err());
}
