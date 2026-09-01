# Manual Release Guide

Publishing to crates.io and npm is intentionally performed by a maintainer from
a trusted local machine. GitHub Actions verifies the release and builds native
artifacts, but it does not hold crates.io or npm registry tokens and never
publishes those packages. The cargo-dist tag workflow may update Homebrew.

## 1. Prepare the release commit

Update the version consistently in:

- `Cargo.toml`
- `ipd/Cargo.toml`, including its `ip-discovery` dependency requirement
- `node/Cargo.toml`
- `node/package.json` and `node/package-lock.json`
- the root `package.json`
- `CHANGELOG.md`

Confirm that the worktree contains only the intended release changes:

```bash
git status --short
git diff --check
```

## 2. Run release gates

```bash
npm ci
npm --prefix node ci
npm run lint
npm test

cargo test -p ip-discovery --no-default-features
cargo test -p ip-discovery --no-default-features --features dns
cargo test -p ip-discovery --no-default-features --features stun
cargo test -p ip-discovery --no-default-features --features http
cargo test -p ip-discovery --no-default-features --features tokio

RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo deny check
cargo publish --dry-run -p ip-discovery
```

Run the network smoke tests separately:

```bash
cargo test --workspace --all-features --test integration -- --ignored --test-threads=1
IP_DISCOVERY_NETWORK_TESTS=1 npm --prefix node test
```

## 3. Build Node.js release artifacts

Run the `Build Release Packages` workflow manually for the release commit. It
verifies the source and uploads native bindings for Linux x64, macOS x64,
macOS arm64, and Windows x64. It does not publish anything.

Download the combined `node-bindings-all` artifact into a temporary directory,
then copy the verified files into `node/`. Using a temporary directory avoids
the extraction failure that occurs when a binding already exists:

```bash
BINDINGS_DIR="$(mktemp -d)"
gh run download RUN_ID --name node-bindings-all --dir "$BINDINGS_DIR"
ls -lh "$BINDINGS_DIR"/*.node
cp "$BINDINGS_DIR"/*.node node/
```

Verify that all four expected native binaries are present, then inspect the npm
package without publishing:

```bash
ls -lh node/*.node
npm pack ./node --dry-run
```

## 4. Publish manually

Authenticate on the trusted release machine:

```bash
cargo login
npm login
```

Publish the Rust library first:

```bash
cargo publish -p ip-discovery
```

Wait until the new library version is visible on crates.io. Then verify and
publish the CLI, whose package depends on that library version:

```bash
cargo publish --dry-run -p ipd
cargo publish -p ipd
```

Publish the npm package only after inspecting `npm pack ./node --dry-run` and the
bundled native binaries:

```bash
npm publish ./node --access public
```

crates.io and npm registry tokens must never be committed or stored in GitHub
Actions secrets for this project.

## 5. Tag and verify

After all packages are available, tag the exact release commit and push the tag:

```bash
git tag -a vVERSION -m "vVERSION"
git push origin vVERSION
```

Use `git tag -s` instead only when GPG signing is configured locally.

The cargo-dist workflow may create the GitHub Release and CLI installers from
the tag and is currently configured to update the Homebrew tap. It never
publishes crates.io or npm packages. Verify crates.io, npm, GitHub Release
artifacts, installers, Homebrew, and a clean installation of each published
package.

Published versions are immutable. Fix a bad release with a new patch version;
do not delete and reuse a version number.

## Automated maintainer workflow

The repository includes a defensive wrapper for the manual process. It checks
manifest versions, Git/CI state, local release gates, Node artifacts, registry
ordering, the cargo-dist GitHub Release, and the Homebrew formula:

```bash
scripts/release.sh all 0.5.1
```

Preview all mutations first:

```bash
scripts/release.sh all 0.5.1 --dry-run
```

Each phase is independently rerunnable, which is useful after authentication or
registry-index delays:

```bash
scripts/release.sh check 0.5.1
scripts/release.sh artifacts 0.5.1
scripts/release.sh publish 0.5.1
scripts/release.sh tag 0.5.1
scripts/release.sh verify 0.5.1
```

`publish` skips versions already present on crates.io or npm. Irreversible
publish and tag steps require typed confirmation unless `--yes` is supplied.
Tags are annotated by default, so GPG is not required; use `--sign-tag` only on
a machine with signing configured. The script never reads, writes, or uploads
registry tokens—it relies on the existing `cargo login`, `npm login`, and `gh`
sessions.
