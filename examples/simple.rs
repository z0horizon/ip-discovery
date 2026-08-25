//! Simple example - get public IP using default configuration

use ip_discovery::{get_ip, get_ipv4};

#[tokio::main]
async fn main() {
    println!("Getting public IP address...\n");

    // Get any IP (v4 or v6)
    match get_ip().await {
        Ok(result) => {
            println!("✓ Public IP: {}", result.ip);
            println!("  Provider:  {}", result.provider);
            println!("  Protocol:  {}", result.protocol);
            println!("  Latency:   {:?}", result.latency);
        }
        Err(e) => {
            eprintln!("✗ Failed to get IP: {}", e);
        }
    }

    println!();

    // Get IPv4 specifically
    println!("Getting IPv4 address...\n");
    match get_ipv4().await {
        Ok(result) => {
            println!("✓ IPv4: {}", result.ip);
            println!("  Provider:  {}", result.provider);
        }
        Err(e) => {
            eprintln!("✗ Failed to get IPv4: {}", e);
        }
    }
}
