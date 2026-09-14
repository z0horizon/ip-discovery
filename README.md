# ip-discovery - Fast public IP lookup

[![Crates.io](https://img.shields.io/crates/v/ip-discovery.svg)](https://crates.io/crates/ip-discovery)
[![docs.rs](https://docs.rs/ip-discovery/badge.svg)](https://docs.rs/ip-discovery)
[![MIT/Apache-2.0](https://img.shields.io/crates/l/ip-discovery.svg)](./LICENSE-MIT)

A Rust library and CLI to detect your public IP address using DNS, STUN, or HTTP — with built-in fallback across trusted providers.

## Why ip-discovery?

Most machines don't know their own public IP. If you're behind NAT, a load balancer, or a cloud VPC, your OS only sees a private address like `10.x.x.x` or `192.168.x.x`. This library solves that — reliably, fast, and with zero configuration.

**Common use cases:**

- **Self-hosted servers with dynamic IPs** — Your home server or office NAS gets a new IP every time the ISP rotates it. Use `ip-discovery` to detect the change and update your DNS record (dynamic DNS), notify clients, or refresh firewall rules — automatically.

- **WebRTC / P2P connection setup** — When building WebRTC applications, you need your public IP to generate SDP offers/answers and ICE candidates. `ip-discovery` uses the same STUN protocol that browsers use, giving you the public-facing address for direct peer connections without relying on a browser environment.

- **NAT traversal & hole punching** — Building a peer-to-peer system (game server, file sharing, VPN)? You need to know your public IP and the type of NAT you're behind before you can punch through it.

- **Server self-registration** — Microservices or edge nodes that spin up in dynamic cloud environments (auto-scaling groups, spot instances) and need to register their public address with a service registry or coordination layer.

- **Security & audit logging** — Record the public IP of the machine at the time of an event for compliance or forensics. Use `Consensus` strategy to cross-verify across multiple providers and guard against a single provider being spoofed.

- **CLI diagnostics** — Quickly check "what IP does the internet see me as?" during debugging, without opening a browser or remembering which `curl` endpoint to hit.

### Why not just `curl` an IP-echo service?

Calling a single HTTP endpoint works for a quick manual check, but falls short in production:

| | HTTP IP-echo services | `ip-discovery` |
|---|---|---|
| **Single point of failure** | If that one service is down or slow, you get nothing | Automatic fallback across 9 providers and 3 protocols |
| **Rate limiting** | Many free services aggressively throttle or block automated requests | DNS and STUN are lightweight UDP queries — far less likely to be throttled than HTTP APIs |
| **Latency** | Full TCP + TLS handshake every time (~200–500ms) | DNS & STUN use raw UDP — typically **<50ms**, 2–3× faster |
| **Result verification** | You trust one provider blindly — it could return stale data or be spoofed | `Consensus` strategy cross-checks across multiple providers |
| **IPv6 support** | Depends on the endpoint; many only return IPv4 | First-class IPv4 and IPv6 support across DNS and STUN |
| **Dependency in code** | Needs shell-out or an HTTP client just to get an IP | Embeddable Rust library, no HTTP dependency needed (DNS + STUN only) |
| **Offline-friendly** | Requires an HTTP-capable environment / TLS stack | DNS and STUN work in minimal environments with just UDP |

> **💡 Note:** Some strict enterprise networks block outbound UDP entirely. In those environments, DNS and STUN won't work.
> Enable the `http` feature to add HTTP-based providers as a fallback — the library will automatically
> try them if UDP-based providers fail.

## CLI Tool — `ipd`

A command-line tool powered by this library. Get your public IP in one command:

```bash
$ ipd
203.0.113.42
```

### Install

**Homebrew (macOS):**

```bash
brew install z0horizon/tap/ipd
```

**Shell (macOS & Linux):**

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/z0horizon/ip-discovery/releases/latest/download/ipd-installer.sh | sh
```

**PowerShell (Windows):**

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/z0horizon/ip-discovery/releases/latest/download/ipd-installer.ps1 | iex"
```

**Cargo:**

```bash
cargo install ipd
```

### CLI Usage

```bash
ipd                         # Plain output; prefer IPv4, fall back to IPv6
ipd -4                      # IPv4 only
ipd -6                      # IPv6 only
ipd -l                      # Local private IP (alias --private)
ipd -n                      # Suppress trailing newline (plain mode)
ipd -q                      # Quiet mode; exit 0 if resolved, 1 on error
ipd -f json                 # JSON output
ipd -f verbose              # Verbose output with provider info
ipd -s race                 # Race all providers, return fastest
ipd -p dns -p stun          # Use only DNS and STUN protocols
ipd --provider google-stun  # Filter by specific provider (can be repeated)
ipd -t 5                    # 5 second timeout
```

---

## Library

### Features

- DNS and STUN via raw UDP sockets (zero network library dependencies)
- HTTP/HTTPS via [reqwest](https://docs.rs/reqwest) (optional)
- Built-in providers from Google, Cloudflare, AWS, and OpenDNS
- IPv4 and IPv6
- Sequential fallback, race, or consensus strategies
- Custom synchronous providers via `BlockingProvider`
- Optional async API via the `tokio`/`async` feature

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
ip-discovery = "0.5"
```

The default API is blocking and does not require Tokio:

```rust
use ip_discovery::blocking::{get_ip, get_ipv4};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = get_ip()?;
    println!("{} via {} in {:?}", result.ip, result.provider, result.latency);

    let v4 = get_ipv4()?;
    println!("IPv4: {}", v4.ip);
    Ok(())
}
```

For the Tokio async API, opt in explicitly:

```toml
[dependencies]
ip-discovery = { version = "0.5", features = ["async"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use ip_discovery::{get_ip, get_ipv4};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = get_ip().await?;
    let v4 = get_ipv4().await?;
    println!("{} / {}", result.ip, v4.ip);
    Ok(())
}
```

The blocking API is callable from a Tokio application too. Because it blocks
the current thread, run network lookups inside `tokio::task::spawn_blocking`,
or use the async API above when Tokio integration is preferred.

To lookup local private IP addresses (synchronously and offline-friendly):

```rust
use ip_discovery::{get_private_ip, get_private_ipv6};

if let Some(local_v4) = get_private_ip() {
    println!("Local IPv4: {}", local_v4);
}
if let Some(local_v6) = get_private_ipv6() {
    println!("Local IPv6: {}", local_v6);
}
```

## Node.js / npm

The library can also be used in Node.js environments via prebuilt native bindings.

### Install

```bash
npm install @z0horizon/ip-discovery
```

### Usage (JavaScript/TypeScript)

```javascript
const { getIp, getIpv4, getIpv6, getPrivateIp, getPrivateIpv6, IpVersion, Strategy, Protocol, BuiltinProvider } = require('@z0horizon/ip-discovery');

// Simple lookup (Public IP)
const result = await getIpv4();
console.log(`Public IP: ${result.ip} (via ${result.provider}, latency: ${result.latencyMs}ms)`);

// Local lookup (Private IP - Synchronous)
console.log(`Local IPv4: ${getPrivateIp()}`);
console.log(`Local IPv6: ${getPrivateIpv6()}`);

// Custom configuration using type-safe enums
const config = {
  timeoutMs: 5000,
  version: IpVersion.V4,
  protocols: [Protocol.Dns, Protocol.Stun],
  strategy: Strategy.Race,
  providers: [BuiltinProvider.CloudflareStun, BuiltinProvider.GoogleDns]
};

const customResult = await getIp(config);
console.log(`IP: ${customResult.ip}`);
```

### Exported Enums

The package exports type-safe enums matching the Rust configuration options:

- **`IpVersion`**: `V4`, `V6`, `Any`
- **`Strategy`**: `First`, `Race`, `Consensus`
- **`Protocol`**: `Dns`, `Http`, `Stun`
- **`BuiltinProvider`**: `GoogleStun`, `GoogleStun1`, `GoogleStun2`, `CloudflareStun`, `GoogleDns`, `CloudflareDns`, `OpenDns`, `CloudflareHttp`, `Aws`


## Configuration

The defaults (Cloudflare STUN → Cloudflare DNS → Google STUN/DNS → OpenDNS; 10s timeout) work well for most cases. If you need more control:

```rust
use ip_discovery::{Config, Strategy, Protocol, BuiltinProvider};
use ip_discovery::blocking::get_ip_with;
use std::time::Duration;

// DNS only, race all DNS providers
let config = Config::builder()
    .protocols(&[Protocol::Dns])
    .strategy(Strategy::Race)
    .timeout(Duration::from_secs(5))
    .build();

let result = get_ip_with(config)?;
```

```rust
// Pick specific providers
let config = Config::builder()
    .providers(&[
        BuiltinProvider::CloudflareDns,
        BuiltinProvider::GoogleStun,
    ])
    .build();
```

```rust
// Consensus — require at least 2 providers to agree
let config = Config::builder()
    .strategy(Strategy::Consensus { min_agree: 2 })
    .build();
```

## Strategies

| Strategy | Description |
|----------|-------------|
| `First` *(default)* | Try providers in order, return first success |
| `Race` | Query all concurrently, return fastest |
| `Consensus { min_agree }` | Require N providers to agree on the same IP |

## Providers

All built-in providers are from tier-1 infrastructure companies:

| Provider | Protocol | IPv4 | IPv6 |
|----------|----------|:----:|:----:|
| Google STUN (`stun.l.google.com`) | STUN | ✅ | ✅ |
| Google STUN 1 (`stun1.l.google.com`) | STUN | ✅ | ✅ |
| Google STUN 2 (`stun2.l.google.com`) | STUN | ✅ | ✅ |
| Cloudflare STUN (`stun.cloudflare.com`) | STUN | ✅ | ✅ |
| Google DNS (`o-o.myaddr.l.google.com`) | DNS | ✅ | ✅ |
| Cloudflare DNS (`whoami.cloudflare`) | DNS | ✅ | ✅ |
| OpenDNS (`myip.opendns.com`) | DNS | ✅ | ❌ |
| Cloudflare HTTP (`1.1.1.1/cdn-cgi/trace`) | HTTP | ✅ | ❌ |
| AWS (`checkip.amazonaws.com`) | HTTP | ✅ | ❌ |

## Cargo Features

| Feature | Default | Description |
|---------|:-------:|-------------|
| `dns` | ✅ | DNS detection (raw UDP, no extra deps) |
| `stun` | ✅ | STUN detection (raw UDP, no extra deps) |
| `http` | ❌ | HTTP detection (pulls in `reqwest` + `rustls`) |
| `tokio` | ❌ | Enable the Tokio-based async API |
| `async` | ❌ | Alias for `tokio` |
| `all` | ❌ | Enable all protocols and the Tokio async API |
| `native-tls` | ❌ | Add reqwest's OS-native TLS backend (requires `http`; rustls remains enabled) |

By default, only DNS and STUN are enabled — zero network library dependencies, fast compile times. To also use HTTP providers:

```toml
ip-discovery = { version = "0.5", features = ["http"] }
```

Or enable everything:

```toml
ip-discovery = { version = "0.5", features = ["all"] }
```

## Timeout behavior

The configured timeout is per provider for `First`. `Race` and `Consensus`
share one caller-visible deadline while their providers run concurrently.
Blocking providers execute on worker threads so the caller returns at the
deadline even if a custom provider ignores its timeout. Rust cannot forcibly
cancel that custom code, so its worker may continue briefly in the background.

Network lookup requires connectivity. In an offline or UDP-blocked environment,
the call returns an error after the applicable deadline; local private-IP
helpers remain available without contacting a remote service.

## Performance

STUN and DNS use raw UDP — no TLS handshake — so they're typically 2–3× faster than HTTP.
Default provider order prioritizes UDP-based protocols with IPv4 + IPv6 support first, then falls back to IPv4-only HTTP providers.

> **💡 Tip:** Latency varies significantly by region and network environment. Run the benchmark on
> your own infrastructure to find the optimal provider and strategy for your use case.

```bash
# Run the benchmark to find the best config for your network
cargo run --example benchmark --all-features
```

## Examples

```bash
cargo run --example blocking
cargo run --example simple --features tokio
cargo run --example custom_providers --features tokio
cargo run --example benchmark --all-features
```

## MSRV

Rust **1.85** or later.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for our upcoming milestones, planned features, and architectural directions.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and required
checks. Report vulnerabilities privately according to
[SECURITY.md](SECURITY.md). Maintainer-only manual publishing steps are in
[RELEASING.md](RELEASING.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your option.
