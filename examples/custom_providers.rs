//! Custom providers example - configure specific providers and strategies

use ip_discovery::{get_ip_with, BuiltinProvider, Config, IpVersion, Protocol, Strategy};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Example 1: DNS only (lightweight, no HTTP dependency needed)
    println!("=== DNS Only ===\n");
    let config = Config::builder()
        .protocols(&[Protocol::Dns])
        .version(IpVersion::V4)
        .build();
    match get_ip_with(config).await {
        Ok(result) => println!("DNS IP: {} ({:?})\n", result.ip, result.latency),
        Err(e) => println!("DNS failed: {}\n", e),
    }

    // Example 2: Pick specific providers
    println!("=== Specific Providers ===\n");
    let config = Config::builder()
        .providers(&[BuiltinProvider::CloudflareDns, BuiltinProvider::GoogleStun])
        .build();
    match get_ip_with(config).await {
        Ok(result) => println!(
            "IP: {} via {} ({:?})\n",
            result.ip, result.provider, result.latency
        ),
        Err(e) => println!("Failed: {}\n", e),
    }

    // Example 3: Race strategy (fastest wins)
    println!("=== Race Strategy ===\n");
    let config = Config::builder()
        .strategy(Strategy::Race)
        .timeout(Duration::from_secs(5))
        .build();
    match get_ip_with(config).await {
        Ok(result) => {
            println!(
                "Fastest: {} via {} ({:?})\n",
                result.ip, result.provider, result.latency
            );
        }
        Err(e) => println!("Race failed: {}\n", e),
    }

    // Example 4: Consensus strategy (multiple providers must agree)
    println!("=== Consensus Strategy (min 2 agree) ===\n");
    let config = Config::builder()
        .strategy(Strategy::Consensus { min_agree: 2 })
        .timeout(Duration::from_secs(10))
        .build();
    match get_ip_with(config).await {
        Ok(result) => {
            println!("Consensus IP: {}\n", result.ip);
        }
        Err(e) => println!("Consensus failed: {}\n", e),
    }

    // List all available providers
    println!("=== Available Providers ===");
    for provider in BuiltinProvider::ALL {
        println!("  - {} ({})", provider, provider.protocol());
    }
}
