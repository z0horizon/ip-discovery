# Blocking API Release Plan (v0.5.0)

## Release decision

Current status: **LOCAL GO / CI PENDING**. The correctness blockers found during review have been fixed and covered by deterministic regression tests. Tagging still requires the cross-platform and publishing gates below.

Recommended version: **0.5.0**. The blocking API is a significant feature and making Tokio optional changes the feature contract for users of `default-features = false`.

## Blocking issues

### P0 — Timeout contract is not enforced for custom blocking providers

Status: **Fixed.** Blocking resolvers now execute providers on worker threads and enforce caller-visible deadlines. The unavoidable limitation that Rust cannot forcibly cancel a running custom provider is documented on `BlockingProvider` and the blocking module.

Originally, the blocking resolver only passed the duration into `BlockingProvider::get_ip`. It now adds a caller-visible deadline around every provider worker.

Release requirement:

- Decide and document one explicit contract:
  - provider-owned timeout, with clear warning that it cannot be enforced; or
  - resolver-owned deadline using worker threads and bounded receive operations.
- Add tests for providers that exceed, ignore, and exactly meet the timeout.
- Define what happens to outstanding `Race` workers after a result is returned.

### P0 — Zero-duration UDP timeout is silently disabled

Status: **Fixed.** DNS and STUN reject zero-duration timeouts before I/O and propagate socket timeout configuration failures.

`UdpSocket::set_read_timeout(Some(Duration::ZERO))` and `set_write_timeout` return an error on supported platforms. The providers now reject zero before socket I/O and propagate timeout-configuration errors.

Release requirement:

- Handle `Duration::ZERO` before socket I/O and return a deterministic timeout error.
- Propagate failures from `set_read_timeout` and `set_write_timeout`.
- Add local UDP tests for `0ms`, `1ms`, delayed response, and no response.

### P1 — Blocking `Race` and `Consensus` use unbounded thread-per-provider execution

Status: **Accepted limitation with bounded caller latency.** Resolver deadlines are enforced, worker panics become provider errors, and the cooperative-cancellation requirement is documented. Built-in provider count remains small; custom providers must honor the timeout.

Every concurrent resolution still spawns one OS thread per matching provider. Remaining `Race` threads may finish after the first success; the behavior and cooperative timeout requirement are now documented.

Release requirement:

- Document a provider-count limit or use scoped/bounded workers.
- Add stress tests that repeatedly race slow providers and observe thread/resource stability.
- Establish a microbenchmark regression threshold.

### P1 — Async and blocking custom-provider configuration can diverge

Status: **Fixed.** Adding either kind of custom provider suppresses built-ins for both resolver modes. A resolver with no providers for its mode returns `NoProvidersForVersion` instead of accessing the network.

Custom providers now suppress built-ins consistently across async and blocking modes.

Release requirement:

- Define whether a `Config` is mode-specific or represents both modes consistently.
- Add tests for async-only custom providers, blocking-only custom providers, protocol filters plus custom providers, and empty provider lists.

### P1 — DNS response correlation is incomplete

Status: **Fixed.** DNS sockets are connected to the configured resolver and responses are checked for transaction ID, response flag, and truncation.

DNS response correlation now checks the random transaction ID and receives only from the connected resolver socket.

Release requirement:

- Verify transaction ID and response/query flags.
- Connect the UDP socket to the resolver or validate the sender returned by `recv_from`.
- Add tests for wrong transaction ID, wrong sender, truncated response, and malformed answer sections.

## Test gate

All of the following must pass on Linux, macOS, and Windows where applicable:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p ip-discovery --no-default-features
cargo test -p ip-discovery --no-default-features --features dns
cargo test -p ip-discovery --no-default-features --features stun
cargo test -p ip-discovery --no-default-features --features http
cargo test -p ip-discovery --no-default-features --features dns,stun
cargo test -p ip-discovery --no-default-features --features tokio
cargo doc --workspace --all-features --no-deps (with RUSTDOCFLAGS=-D warnings)
npm --prefix node test
```

Add deterministic, local tests for:

- `First`: ordered fallback, all failures, zero providers, timeout boundary.
- `Race`: fast success, failures before success, all failures, slow workers after return, provider panic.
- `Consensus`: exact threshold, impossible threshold, tie, fastest member of winning group, mixed failures.
- IP versions: V4/V6/Any filtering and a provider returning the wrong IP family.
- DNS/STUN: zero timeout, malformed packet, wrong transaction ID, unexpected sender, IPv4/IPv6 selection.
- HTTP: non-2xx, delayed headers/body, invalid body, wrong IP family, connection failure.
- Feature matrix: public API availability with and without `tokio`.

Network smoke tests should remain separate from deterministic CI tests and run serially with bounded timeouts.

## Performance gate

Baseline measured on 2026-08-25 from the current development machine:

| Scenario | Blocking | Async |
|:--|--:|--:|
| Resolver overhead, `First`, one immediate mock (p50) | 17.13 µs | 3.38 µs |
| Resolver overhead, `Race`, four immediate mocks (p50) | 28.71 µs | 3.63 µs |
| Resolver overhead, `Consensus`, four immediate mocks (p50) | 38.04 µs | 4.13 µs |
| Cloudflare DNS IPv4, 15 runs (mean) | 36.03 ms | 33.05 ms |
| Cloudflare STUN IPv4, 15 runs (mean) | 34.35 ms | 37.69 ms |
| AWS HTTP IPv4, 15 runs (mean) | 191.84 ms | 190.75 ms |

Release thresholds:

- `First` blocking overhead p50 <= 35 µs for an immediate mock provider. The worker boundary is intentional so custom providers cannot block caller-visible deadlines.
- `Race`/`Consensus` blocking p50 <= 75 µs for four immediate providers.
- No more than 20% median regression against the checked-in baseline for each strategy.
- 100% success in a 15-run IPv4 smoke test for at least one DNS, STUN, and HTTP provider on the release runner.
- DNS/STUN-only blocking example remains below 1 MiB stripped on the reference macOS build. Current release binary is about 597 KiB; async/all-features example is about 5.4 MiB.

Benchmarks must use release builds, warm-up runs, multiple samples, and report p50/p95 rather than a single timing.

## Compatibility and documentation

- Add a migration note: async users with `default-features = false` must enable `tokio` or `async` explicitly.
- Clearly distinguish:
  - blocking DNS/STUN, which does not require Tokio;
  - blocking HTTP through `reqwest`, whose dependency tree still includes Tokio/Hyper internally.
- Document timeout semantics and worst-case duration for `First` and `Consensus`.
- Add blocking examples for custom providers and all strategies.
- Update README feature table and changelog.

## CI and publishing changes

- `.github/workflows/publish.yml` is now a manually dispatched verify/build workflow. It runs the feature matrix and uploads cross-platform Node bindings but never publishes to crates.io or npm.
- Add deterministic blocking protocol tests to normal CI instead of relying only on ignored network tests.
- `deny.toml` defines the dependency, source, and license policy; `cargo deny check` is required locally and in CI.
- Run `cargo publish --dry-run -p ip-discovery` before tagging. The `ipd`
  dry-run can only resolve version `0.5` after the library is visible in the
  crates.io index, so run it between publishing the library and the CLI.
- Run the cargo-dist PR workflow and inspect artifacts for all configured targets.

## Release sequence

1. Fix P0 issues and add regression tests.
2. Resolve or explicitly accept/document P1 issues.
3. Run full test, lint, docs, security, feature, and performance gates.
4. Update versions to `0.5.0` consistently in Rust and npm manifests.
5. Add `CHANGELOG.md` migration notes and blocking API examples.
6. Open a release-candidate PR and exercise cargo-dist without publishing.
7. Build and smoke-test CLI artifacts on Linux, macOS (x64/arm64), and Windows.
8. Run the library Cargo publish dry-run, the npm pack/publish dry-runs, and
   build all Node native bindings. Publishing crates.io and npm packages is
   performed manually by the maintainer according to `RELEASING.md`; CI must
   not hold those registry tokens or publish those packages. Cargo-dist may
   still update the configured Homebrew tap after tagging.
9. Tag `v0.5.0` only after all required checks are green.
10. Verify crates.io, npm, GitHub Release, installers, and Homebrew formula after publishing.
11. Run post-release installation smoke tests from clean environments.

## Rollback

- Do not reuse a published version. Fix forward with `0.5.1` if a package was already published.
- Yank the affected crates.io version only for a serious correctness/security issue.
- Deprecate the affected npm version and point users to the fixed version.
- Preserve GitHub artifacts and publish a clear advisory/migration note.
