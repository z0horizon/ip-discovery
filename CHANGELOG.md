# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.1] - 2026-09-14

### Added

- Added `FromStr` implementations for `Protocol`, `IpVersion`, and `BuiltinProvider`.
- Added `is_ipv4()` and `is_ipv6()` convenience helper methods to `ProviderResult`.
- Added `--provider <PROVIDER>` filter argument to `ipd` CLI to select specific providers.
- Added `-n, --no-newline` flag to `ipd` CLI to suppress trailing newlines in plain mode.
- Added `-q, --quiet` flag to `ipd` CLI for silent exit-code based scripting and health checks.

### Changed

- Updated repository links, author metadata, and package scopes to `z0horizon`.

## [0.5.0] - 2026-08-25

### Added

- Added a synchronous `blocking` API for DNS, STUN, HTTP, and custom
  `BlockingProvider` implementations without requiring a Tokio runtime.
- Added blocking `First`, `Race`, and `Consensus` strategies and a blocking CLI.
- Added deterministic blocking, timeout, malformed-packet, offline, and feature
  matrix coverage.
- Added contributor, security, and maintainer-only manual release guidance.

### Changed

- **BREAKING:** Tokio is no longer enabled by default. Enable `tokio` or its
  `async` alias to retain the top-level async functions, `Provider`, and async
  `Resolver` API.
- **BREAKING:** The `ipd` CLI now uses the blocking implementation internally.
- `IpVersion::Any` prefers IPv4 when both address families are available.
- crates.io and npm publishing are performed manually; CI only verifies and
  builds release artifacts.

### Fixed

- Enforced caller-visible timeouts for blocking providers, including custom
  providers and zero-duration DNS/STUN requests.
- Validated DNS response source, transaction ID, response flags, truncation,
  and requested IP family.
- Validated STUN success-response type, transaction ID, declared message
  length, attribute boundaries, and padding.
- Propagated OS randomness failures instead of sending predictable DNS/STUN
  transaction IDs.
- Made Node.js network tests explicitly opt-in so failures cannot be silently
  converted into passing tests.

## [0.4.1] - 2026-07-19

### Added

- **NPM Package documentation:** Created a dedicated, Node.js-focused `README.md` for the `@z0horizon/ip-discovery` package.

### Fixed

- **NPM Package metadata:** Included `LICENSE-MIT`, `LICENSE-APACHE`, author, homepage, and repository fields in the package distribution to fix missing details on npmjs.com.

## [0.4.0] - 2026-07-18

### Added

- **Node.js Native Bindings:** Added `@z0horizon/ip-discovery` npm package built using `napi-rs`, providing high-performance, type-safe bindings for JavaScript/TypeScript environments.
- **Local Private IP Lookup:** Added support for synchronous, offline-friendly private IP address lookup (IPv4 and IPv6) via `get_private_ip()` / `get_private_ipv6()` in Rust, `getPrivateIp()` / `getPrivateIpv6()` in JS, and `-l` / `--private` in the CLI.
- **Husky Pre-commit Hooks:** Added local pre-commit hooks to enforce formatting (`cargo fmt`), strict lints (`clippy`), and all unit tests locally.

### Fixed

- **CLI Private IP Formatting:** Extended the existing CLI formatting options (`-f json` and `-f verbose`) to support formatting local private IP output.
- **macOS compilation support:** Configured `.cargo/config.toml` with macOS linker arguments (`-undefined dynamic_lookup`) to prevent dynamic library compilation errors when building Node bindings.
- **MSRV resolver & HTTP/2 multiplexing:** Fixed CI builds by enabling MSRV-aware resolver and disabling HTTP/2 multiplexing.
- **CI build issues:** Resolved CI build errors by compiling native Node bindings on the runner prior to running Javascript tests.
- **Clippy lints:** Fixed redundant closure warnings in the CLI tool argument parsing.
- **Test race conditions:** Fixed test runner timeout assertion failures on fast CI runners by using a guaranteed 0ms timeout.

## [0.3.0] - 2026-03-26

### Changed

- **BREAKING:** Removed `async_trait` and `tracing` dependencies
- **BREAKING:** All public enums are now `#[non_exhaustive]`
- **BREAKING:** Removed unused `Error::Timeout` and `Error::NoProviders` variants
- **BREAKING:** `Error::ConsensusNotReached` now includes provider errors
- **BREAKING:** Removed inner STUN timeout; resolver timeout is now the single source of truth
- Updated `getrandom` to v0.4

### Fixed

- Corrected DNS buffer size RFC reference (RFC 8020 → RFC 6891)

### Documentation

- Added limitations and security notes to DNS and STUN modules

## [0.2.0] - 2026-03-18

### Changed

- **BREAKING:** `http` feature is no longer enabled by default. Only `dns` and `stun` are default, keeping the crate zero-dependency for network libraries. Enable HTTP explicitly with `features = ["http"]` or use `features = ["all"]`.
- Added `all` convenience feature to enable all protocols

### Fixed

- Fix `ancount` read from wrong DNS header offset (was reading `qdcount` at bytes 4-5 instead of `ancount` at bytes 6-7)
- Fix question section skipping to handle compression pointers and multiple questions via `qdcount` loop
- Fix infinite loop in TXT record parsing when text-length exceeds `rdlength` bounds

## [0.1.5] - 2026-03-18 [yanked]

### Fixed

- Fix `ancount` read from wrong DNS header offset (was reading `qdcount` at bytes 4-5 instead of `ancount` at bytes 6-7)
- Fix question section skipping to handle compression pointers and multiple questions via `qdcount` loop
- Fix infinite loop in TXT record parsing when text-length exceeds `rdlength` bounds

## [0.1.4] - 2026-03-17

### Fixed

- Remove `doc_auto_cfg` feature gate (removed in Rust 1.92, merged into `doc_cfg`)

## [0.1.3] - 2026-03-17

### Fixed

- Add `docs.rs` metadata to enable documentation generation with all features
- Add `doc_auto_cfg` for automatic feature gate annotations in documentation

## [0.1.2] - 2026-03-17

### Fixed

- Bump MSRV from 1.75 to 1.85 (required by `reqwest` transitive dependencies)

## [0.1.1] - 2026-03-17 [yanked]

### Fixed

- Incorrect MSRV (set to 1.82, but dependencies require 1.83+)

## [0.1.0] - 2026-03-17

### Added

- Multi-protocol public IP detection: DNS, HTTP/HTTPS, STUN (RFC 5389)
- Built-in providers from trusted sources: Google, Cloudflare, AWS, OpenDNS
- IPv4 and IPv6 support with per-provider version capability
- Three resolution strategies: `First` (sequential fallback), `Race` (fastest wins), `Consensus` (multi-provider agreement)
- Builder-pattern configuration via `Config::builder()`
- Convenience functions: `get_ip()`, `get_ipv4()`, `get_ipv6()`, `get_ip_with()`
- Custom provider support via the `Provider` trait
- Performance-optimized default provider order (UDP-first, IPv4+IPv6 preferred)
- Zero-dependency DNS and STUN implementations (raw UDP sockets)
- Cargo features: `dns`, `stun`, `http` (all default), `native-tls` (optional)
- Structured diagnostics via `tracing`
- Examples: `simple`, `custom_providers`, `benchmark`
