# Contributing to ip-discovery

Thanks for helping improve `ip-discovery`. Bug reports, protocol compatibility
reports, documentation fixes, tests, and code contributions are welcome.

## Development setup

The project requires:

- Rust 1.85 or newer
- Node.js 20 or newer for the Node.js bindings
- npm
- `cargo-deny` for the dependency policy check

Clone the repository and install the Node.js dependencies:

```bash
npm ci
npm --prefix node ci
cargo install cargo-deny --locked
```

## Making changes

1. Create a focused branch from `main`.
2. Keep unrelated formatting or dependency changes out of the same pull request.
3. Add deterministic tests for behavior changes and regressions.
4. Update public API documentation and `CHANGELOG.md` when appropriate.
5. Run the checks below before opening a pull request.

Blocking providers must honor the timeout passed to
`BlockingProvider::get_ip`. The resolver enforces a caller-visible deadline,
but Rust cannot forcibly stop provider work that is already running on a
thread. Tests for network protocols should use local sockets or test doubles;
public network checks belong in ignored smoke tests.

## Required checks

```bash
npm run lint
npm test

cargo test -p ip-discovery --no-default-features
cargo test -p ip-discovery --no-default-features --features dns
cargo test -p ip-discovery --no-default-features --features stun
cargo test -p ip-discovery --no-default-features --features http
cargo test -p ip-discovery --no-default-features --features tokio

RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo deny check
```

To verify the minimum supported Rust version:

```bash
rustup toolchain install 1.85.0 --profile minimal
cargo +1.85.0 check --workspace --all-features
cargo +1.85.0 check -p ip-discovery --no-default-features --features dns,stun
```

Network smoke tests require internet access and should be run serially:

```bash
cargo test --workspace --all-features --test integration -- --ignored --test-threads=1
```

## Pull requests

A pull request should explain the user-visible behavior, compatibility impact,
and test coverage. CI must pass on the supported platforms before merge. Avoid
committing generated native `.node` binaries.

Security vulnerabilities should not be reported in a public issue. Follow
[SECURITY.md](SECURITY.md) instead.

By participating, you agree to follow the project
[Code of Conduct](CODE_OF_CONDUCT.md).

Maintainer release instructions are documented in [RELEASING.md](RELEASING.md).
