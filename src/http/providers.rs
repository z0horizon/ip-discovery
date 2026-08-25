//! Built-in HTTP providers

use super::{parse_cloudflare_trace, HttpProvider};
use crate::provider::BoxedBlockingProvider;
#[cfg(feature = "tokio")]
use crate::provider::BoxedProvider;

/// Cloudflare trace endpoint
pub fn cloudflare() -> HttpProvider {
    HttpProvider::new("Cloudflare", "https://1.1.1.1/cdn-cgi/trace")
        .with_parser(parse_cloudflare_trace)
}

/// AWS checkip service
pub fn aws() -> HttpProvider {
    HttpProvider::new("AWS", "https://checkip.amazonaws.com")
}

/// List all available HTTP provider names
pub fn provider_names() -> &'static [&'static str] {
    &["Cloudflare", "AWS"]
}

/// Get default blocking HTTP providers
pub fn default_blocking_providers() -> Vec<BoxedBlockingProvider> {
    vec![Box::new(cloudflare()), Box::new(aws())]
}

/// Get default async HTTP providers
#[cfg(feature = "tokio")]
pub fn default_providers() -> Vec<BoxedProvider> {
    vec![Box::new(cloudflare()), Box::new(aws())]
}
