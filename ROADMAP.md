# ip-discovery Roadmap

This document outlines the planned milestones, upcoming features, and long-term architectural directions for the `ip-discovery` library, CLI (`ipd`), and language bindings.

> **Note**: This roadmap reflects current project priorities and directions. Priorities may evolve based on real-world user feedback, community contributions, and ecosystem needs. Timelines are targets and do not represent rigid release commitments.

---

## Current Release: v0.5.1

- [x] **Multi-Protocol Discovery**: DNS (raw UDP), STUN (RFC 5389 UDP), and HTTP (`reqwest`).
- [x] **Flexible Runtime**: Zero-runtime blocking API by default; optional Tokio-based async API via `features = ["tokio"]`.
- [x] **Resolution Strategies**: `First` (sequential fallback), `Race` (fastest responder), and `Consensus` (majority agreement).
- [x] **Local & Public Addressing**: Public IPv4/IPv6 lookup and offline-safe local private IP detection.
- [x] **CLI Tool (`ipd`)**: Comprehensive output formats (`plain`, `json`, `verbose`) and multi-platform packaging (Homebrew, Cargo, Shell/PowerShell installers, cargo-dist).
- [x] **Node.js Bindings**: High-performance N-API bindings via `@z0horizon/ip-discovery`.

---

## Milestone v0.6.0: Automation & CLI Watcher

Focus on automated IP tracking and local system integration for home servers, NAS devices, edge nodes, and DevOps environments.

- [ ] **CLI Watch Mode (`ipd watch`)**
  - Add continuous monitoring with configurable interval (`--interval <seconds>`, default: 60s).
  - Low-overhead polling loop with graceful shutdown on `SIGINT` / `SIGTERM` (`Ctrl+C`).
  - Differentiate between transient network glitches and confirmed IP changes.

- [ ] **Event Hooks & Shell Execution**
  - Support executing custom commands on IP change (`--on-change "notify.sh"` or `--exec "<cmd>"`).
  - Pass previous IP, new IP, protocol, and timestamp as environment variables or arguments.

- [ ] **Webhook Notifications**
  - Built-in webhook dispatch (`--webhook <url>`) on IP change.
  - JSON payload compatible with generic HTTP endpoints, Slack, Discord, and Telegram webhooks.

- [ ] **Network Interface Binding**
  - Allow binding to a specific network interface (e.g. `eth0`, `wlan0`, `en0`) or source IP address.
  - Platform-aware socket configuration (`SO_BINDTODEVICE` on Linux, `IP_BOUND_IF` on macOS/BSD).

---

## Milestone v0.7.0: Ecosystem Expansion & Core Optimization

Focus on broadening language support and streamlining core dependency trees.

- [ ] **Official Python SDK (`ip-discovery` on PyPI)**
  - Native Python bindings powered by `PyO3` and `maturin`.
  - Type hints (`.pyi` stubs), synchronous and `asyncio` support.
  - Pre-built binary wheels for Linux (x86_64, aarch64), macOS (universal2 / Apple Silicon), and Windows.

- [ ] **Go Module Wrapper**
  - Idiomatic Go module powered by C-ABI / CGO bindings.

- [ ] **Java / JVM SDK**
  - Native bindings via JNI or Java Foreign Function & Memory API (Project Panama).

- [ ] **Lightweight Sync HTTP Client**
  - Evaluate a minimal sync HTTP client to eliminate the internal Tokio/Hyper dependency in blocking mode when HTTP fallback is enabled.
  - Keep binary size stripped under 1 MiB even with HTTP support.

- [ ] **Enhanced Node.js Bindings**
  - Support interface binding and watch event emitters in `@z0horizon/ip-discovery`.

---

## Backlog & Future Explorations

Features under exploration that may be prioritized according to community interest:

- [ ] **Dynamic DNS (DDNS) Provider Integration**
  - Direct updates to Cloudflare DNS, DuckDNS, AWS Route53, and Namecheap without external tooling.
- [ ] **Port Mapping & Traversal (NAT-PMP / PCP)**
  - Support RFC 6886 (NAT-PMP) and RFC 6887 (PCP) for automatic router port mapping alongside IP lookup.
- [ ] **Advanced NAT Behavior Diagnostics**
  - Investigate RFC 5780 STUN tests to classify NAT types (Full Cone, Restricted, Symmetric NAT) using compliant public STUN servers.

---

## Contributing to the Roadmap

Have a feature request or want to contribute to an upcoming milestone?
- Join the discussion or open an issue on [GitHub Issues](https://github.com/z0horizon/ip-discovery/issues).
- Check [CONTRIBUTING.md](CONTRIBUTING.md) to get started with local development.
