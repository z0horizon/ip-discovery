//! Built-in STUN providers

use super::StunProvider;
use crate::provider::BoxedBlockingProvider;
#[cfg(feature = "tokio")]
use crate::provider::BoxedProvider;

/// Google STUN server
pub fn google() -> StunProvider {
    StunProvider::new("Google STUN", "stun.l.google.com", 19302)
}

/// Google STUN server 1
pub fn google1() -> StunProvider {
    StunProvider::new("Google STUN 1", "stun1.l.google.com", 19302)
}

/// Google STUN server 2
pub fn google2() -> StunProvider {
    StunProvider::new("Google STUN 2", "stun2.l.google.com", 19302)
}

/// Cloudflare STUN server
pub fn cloudflare() -> StunProvider {
    StunProvider::new("Cloudflare STUN", "stun.cloudflare.com", 3478)
}

/// List all available STUN provider names
pub fn provider_names() -> &'static [&'static str] {
    &[
        "Google STUN",
        "Google STUN 1",
        "Google STUN 2",
        "Cloudflare STUN",
    ]
}

/// Get default blocking STUN providers
pub fn default_blocking_providers() -> Vec<BoxedBlockingProvider> {
    vec![
        Box::new(google()),
        Box::new(cloudflare()),
        Box::new(google1()),
        Box::new(google2()),
    ]
}

/// Get default async STUN providers
#[cfg(feature = "tokio")]
pub fn default_providers() -> Vec<BoxedProvider> {
    vec![
        Box::new(google()),
        Box::new(cloudflare()),
        Box::new(google1()),
        Box::new(google2()),
    ]
}
