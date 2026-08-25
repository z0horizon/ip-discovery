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

Download all `bindings-*` artifacts and copy the `.node` files into `node/`.
With the GitHub CLI, replace `RUN_ID` with the completed workflow run:

```bash
gh run download RUN_ID --dir node/artifacts
find node/artifacts -name '*.node' -exec cp {} node/ \;
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
git tag -s vVERSION -m "vVERSION"
git push origin vVERSION
```

The cargo-dist workflow may create the GitHub Release and CLI installers from
the tag and is currently configured to update the Homebrew tap. It never
publishes crates.io or npm packages. Verify crates.io, npm, GitHub Release
artifacts, installers, Homebrew, and a clean installation of each published
package.

Published versions are immutable. Fix a bad release with a new patch version;
do not delete and reuse a version number.
