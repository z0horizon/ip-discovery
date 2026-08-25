//! Synchronous (Blocking) example - get public IP without Tokio runtime
//!
//! Run with:
//!   cargo run --example blocking --no-default-features --features dns,stun

use ip_discovery::blocking::{get_ip, get_ip_with, get_ipv4};
use ip_discovery::{BuiltinProvider, Config, IpVersion, Protocol, Strategy};
use std::time::Duration;

fn main() {
    println!("=== Zero-Dependency Synchronous IP Discovery ===\n");

    // 1. Quick lookup using default configuration (First success)
    println!("1. Quick public IP lookup:");
    match get_ip() {
        Ok(result) => {
            println!("   ✓ IP:       {}", result.ip);
            println!("   ✓ Provider: {}", result.provider);
            println!("   ✓ Protocol: {}", result.protocol);
            println!("   ✓ Latency:  {:?}\n", result.latency);
        }
        Err(e) => eprintln!("   ✗ Failed: {e}\n"),
    }

    // 2. Lookup IPv4 specifically
    println!("2. IPv4 specific lookup:");
    match get_ipv4() {
        Ok(result) => println!("   ✓ IPv4:     {}\n", result.ip),
        Err(e) => eprintln!("   ✗ Failed: {e}\n"),
    }

    // 3. Race strategy (concurrent threads, fastest provider wins)
    println!("3. Race strategy (DNS & STUN concurrently):");
    let race_config = Config::builder()
        .protocols(&[Protocol::Dns, Protocol::Stun])
        .strategy(Strategy::Race)
        .timeout(Duration::from_secs(5))
        .build();

    match get_ip_with(race_config) {
        Ok(result) => {
            println!(
                "   ✓ Fastest:  {} (via {} over {})",
                result.ip, result.provider, result.protocol
            );
            println!("   ✓ Latency:  {:?}\n", result.latency);
        }
        Err(e) => eprintln!("   ✗ Race failed: {e}\n"),
    }

    // 4. Custom providers selection
    println!("4. Specific providers selection:");
    let custom_config = Config::builder()
        .providers(&[BuiltinProvider::CloudflareDns, BuiltinProvider::GoogleStun])
        .version(IpVersion::V4)
        .build();

    match get_ip_with(custom_config) {
        Ok(result) => println!("   ✓ IP:       {} (via {})\n", result.ip, result.provider),
        Err(e) => eprintln!("   ✗ Failed: {e}\n"),
    }

    // 5. Local private IP addresses
    println!("5. Local private IP addresses (no network traffic):");
    if let Some(ip) = ip_discovery::get_private_ip() {
        println!("   ✓ Private IPv4: {ip}");
    }
    if let Some(ip) = ip_discovery::get_private_ipv6() {
        println!("   ✓ Private IPv6: {ip}");
    }
}
