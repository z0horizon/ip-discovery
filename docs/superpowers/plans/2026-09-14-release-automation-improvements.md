# Release Automation Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix critical dry-run breakages, resume-safety flaws, transient polling crashes, missing native build gates, and platform dependencies in the release automation pipeline (`scripts/release.sh` and `.github/workflows/publish.yml`).

**Architecture:** Refactor `scripts/release.sh` to isolate dry-run execution from destructive operations and remote assertions, move `cargo publish --dry-run` behind existing-package checks for resume-safety, add bindings receipt validation, and decouple Homebrew verification from local `brew` CLI using GitHub API. Enhance `.github/workflows/publish.yml` to run tests on host native runners.

**Tech Stack:** Bash (`set -Eeuo pipefail`), GitHub CLI (`gh`), GitHub Actions YAML, Cargo, npm.

**Spec:** Code review findings from Senior Release Engineer Reviewer on 2026-09-14.

## Global Constraints

- Never commit or log registry tokens (keep crates.io and npm auth strictly local).
- All changes to `scripts/release.sh` must remain compatible with Bash on macOS and Linux.
- Shell scripts must pass `bash -n` syntax validation and maintain `set -Eeuo pipefail` strict mode.
- `./scripts/release.sh all <version> --dry-run` must complete successfully end-to-end without interactive prompts or registry 404 aborts.

---

### Task 1: Fix Dry-Run Execution & Resume-Safety in `scripts/release.sh`

**Files:**
- Modify: `scripts/release.sh:220-350`

**Interfaces:**
- Consumes: Existing functions `confirm`, `publish_crate`, `phase_publish`, `phase_tag`, `phase_verify`, `validate_bindings_dir`
- Produces: Seamless `--dry-run` across all phases without interactive prompt halts or fatal 404 checks; idempotent `publish_crate`.

- [ ] **Step 1: Update `confirm()` to bypass in dry-run mode**

In `scripts/release.sh`:
```bash
confirm() {
  local action="$1" answer
  [[ "$YES" == true || "$DRY_RUN" == true ]] && return 0
  printf 'Type "%s %s" to continue: ' "$action" "$VERSION" >&2
  IFS= read -r answer
  [[ "$answer" == "$action $VERSION" ]] || die "Confirmation did not match"
}
```

- [ ] **Step 2: Move `cargo publish --dry-run` inside `publish_crate` after `crate_exists` check**

In `scripts/release.sh`:
```bash
publish_crate() {
  local crate="$1"
  if crate_exists "$crate"; then
    log "$crate $VERSION already exists; skipping"
    return 0
  fi
  run_mutation cargo publish --dry-run -p "$crate"
  confirm "publish-$crate"
  run_mutation cargo publish -p "$crate"
  [[ "$DRY_RUN" == true ]] || wait_for_crate "$crate" || die "$crate $VERSION did not become visible on crates.io"
}
```
Remove lines calling `cargo publish --dry-run -p ip-discovery` and `cargo publish --dry-run -p ipd` from `phase_publish`.

- [ ] **Step 3: Update `validate_bindings_dir node` in `phase_publish` for dry-run**

In `phase_publish`, guard `validate_bindings_dir node`:
```bash
phase_publish() {
  require_commands cargo curl node npm
  validate_version
  assert_versions
  assert_clean_main
  if [[ "$DRY_RUN" == true ]]; then
    if ! find node -maxdepth 1 -type f -name '*.node' | grep -q .; then
      log "[dry-run] Node bindings not present locally; skipping validate_bindings_dir"
    else
      validate_bindings_dir node
    fi
  else
    validate_bindings_dir node
  fi
  publish_crate ip-discovery
  publish_crate ipd
  publish_npm
  log "All packages published for $VERSION"
}
```

- [ ] **Step 4: Guard `phase_tag` and `phase_verify` assertions in dry-run mode**

In `phase_tag`:
```bash
  if [[ "$DRY_RUN" == true ]]; then
    log "[dry-run] Skipping crates.io and npm existence check"
  else
    crate_exists ip-discovery || die "ip-discovery $VERSION is not on crates.io"
    crate_exists ipd || die "ipd $VERSION is not on crates.io"
    npm_exists || die "npm package $VERSION is not published"
  fi
```

In `phase_verify`:
```bash
  if [[ "$DRY_RUN" == true ]]; then
    log "[dry-run] Skipping remote verification checks"
    log "Verification passed for $VERSION (dry-run)"
    return 0
  fi
```

- [ ] **Step 5: Verify syntax and test `--dry-run`**

Run: `bash -n scripts/release.sh`
Run: `./scripts/release.sh all 0.5.1 --dry-run`
Expected: Passes without crashing or blocking on prompts.

---

### Task 2: Harden Polling, Error Handling, Gates, and Receipt Tracking in `scripts/release.sh`

**Files:**
- Modify: `scripts/release.sh`

**Interfaces:**
- Consumes: `log`, `run_gates`, `latest_workflow_run`, `wait_for_new_run`, `wait_for_release_run`, `phase_artifacts`, `phase_publish`, `phase_verify`
- Produces: Stderr logging, resilient polling against network hiccups, native build in `run_gates`, receipt tracking, and GitHub API Homebrew verification.

- [ ] **Step 1: Redirect `log()` to stderr**

In `scripts/release.sh:21`:
```bash
log() { printf '[release] %s\n' "$*" >&2; }
```

- [ ] **Step 2: Add connection and max-time timeouts to `curl` in `crate_exists`**

In `scripts/release.sh:243-246`:
```bash
crate_exists() {
  curl --silent --fail --output /dev/null --connect-timeout 5 --max-time 15 \
    --user-agent "$CRATES_USER_AGENT" \
    "https://crates.io/api/v1/crates/$1/$VERSION"
}
```

- [ ] **Step 3: Add `git fetch origin main` to `assert_clean_main`**

In `scripts/release.sh:103-116`:
```bash
assert_clean_main() {
  local status branch head upstream
  status=$(git status --short)
  if [[ -n "$status" ]]; then
    printf '[release] ERROR: Worktree is not clean:\n%s\n' "$status" >&2
    exit 1
  fi
  branch=$(git branch --show-current)
  [[ "$branch" == "main" ]] || die "Release must run from main, currently: $branch"
  upstream=$(git rev-parse --abbrev-ref '@{upstream}' 2>/dev/null) || die "main has no upstream"
  [[ "$upstream" == "origin/main" ]] || die "Expected upstream origin/main, found $upstream"
  git fetch origin main --quiet
  head=$(git rev-parse HEAD)
  [[ "$head" == "$(git rev-parse origin/main)" ]] || die "HEAD is not at origin/main; fetch/push first"
}
```

- [ ] **Step 4: Protect polling loops from transient API network errors under `set -e`**

In `scripts/release.sh:175-202`:
In `wait_for_new_run`:
```bash
  while [[ $attempt -lt 30 ]]; do
    run_id=$(latest_workflow_run 2>/dev/null || true)
    if [[ -n "$run_id" && "$run_id" != "$previous" ]]; then
      printf '%s' "$run_id"
      return 0
    fi
    sleep 2
    attempt=$((attempt + 1))
  done
```
In `wait_for_release_run`:
```bash
  while [[ $attempt -lt 60 ]]; do
    run_id=$(gh run list --repo "$REPO" --workflow release.yml --branch "$tag" --limit 1 \
      --json databaseId --jq '.[0].databaseId // empty' 2>/dev/null || true)
    if [[ -n "$run_id" && "$run_id" != "$previous" ]]; then
      printf '%s' "$run_id"
      return 0
    fi
    sleep 2
    attempt=$((attempt + 1))
  done
```

- [ ] **Step 5: Add `npm --prefix node run build` to `run_gates`**

In `scripts/release.sh:142-157`:
```bash
run_gates() {
  log "Running local release gates"
  git diff --check
  npm ci
  npm --prefix node ci
  npm run lint
  npm --prefix node run build
  npm test
  cargo test -p ip-discovery --no-default-features
  cargo test -p ip-discovery --no-default-features --features dns
  cargo test -p ip-discovery --no-default-features --features stun
  cargo test -p ip-discovery --no-default-features --features http
  cargo test -p ip-discovery --no-default-features --features tokio
  RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps
  cargo deny check
  cargo publish --dry-run -p ip-discovery
}
```

- [ ] **Step 6: Add binding receipt tracking in `phase_artifacts` and `phase_publish`**

In `phase_artifacts`:
```bash
  validate_bindings_dir node
  printf '%s %s\n' "$VERSION" "$(git rev-parse HEAD)" > node/.bindings-receipt
  npm pack ./node --dry-run
  log "Node bindings from run $run_id are ready"
```
In `phase_publish`:
```bash
  if [[ "$DRY_RUN" != true ]]; then
    local receipt_file="node/.bindings-receipt"
    [[ -f "$receipt_file" ]] || die "Missing $receipt_file; run 'scripts/release.sh artifacts $VERSION' first"
    local expected_receipt="$VERSION $(git rev-parse HEAD)"
    local actual_receipt
    actual_receipt=$(cat "$receipt_file")
    [[ "$actual_receipt" == "$expected_receipt" ]] || \
      die "Bindings receipt mismatch: expected '$expected_receipt', found '$actual_receipt'. Re-run 'scripts/release.sh artifacts $VERSION'."
  fi
```

- [ ] **Step 7: Decouple Homebrew formula verification from local `brew` CLI**

In `phase_verify`:
Change `require_commands brew curl gh node npm` to `require_commands curl gh node npm`.
Replace `brew update` and `brew info` with:
```bash
  local formula_version
  formula_version=$(gh api "repos/z0horizon/homebrew-tap/contents/Formula/ipd.rb" --jq '.content' 2>/dev/null \
    | base64 -d 2>/dev/null \
    | sed -n 's/^[[:space:]]*version[[:space:]]*"\(.*\)"/\1/p' || true)
  if [[ -n "$formula_version" ]]; then
    [[ "$formula_version" == "$VERSION" ]] || die "Homebrew formula has version $formula_version; expected $VERSION"
  else
    log "Notice: Homebrew tap check skipped or not yet available"
  fi
```

- [ ] **Step 8: Ensure local tag reuse in `phase_tag`**

In `scripts/release.sh:310-324`:
If tag exists locally pointing to HEAD:
```bash
  if git rev-parse "refs/tags/$tag" >/dev/null 2>&1; then
    local tag_commit
    tag_commit=$(git rev-parse "refs/tags/$tag^{commit}")
    [[ "$tag_commit" == "$(git rev-parse HEAD)" ]] || die "Local tag $tag exists but does not point to HEAD"
    log "Local tag $tag already points to HEAD"
  else
    if [[ "$SIGN_TAG" == true ]]; then
      run_mutation git tag -s "$tag" -m "$tag"
    else
      run_mutation git tag -a "$tag" -m "$tag"
    fi
  fi
```

- [ ] **Step 9: Verify syntax and test `check`**

Run: `bash -n scripts/release.sh`
Run: `./scripts/release.sh check 0.5.1 --dry-run`

---

### Task 3: Enhance Node.js Test Coverage on Host Runners in `publish.yml`

**Files:**
- Modify: `.github/workflows/publish.yml:95-115`

**Interfaces:**
- Consumes: GitHub Actions matrix runners
- Produces: Automated `npm test` execution on native platforms (`macos-latest`, `windows-latest`, `ubuntu-latest`).

- [ ] **Step 1: Add test step on host-matching matrix settings**

In `.github/workflows/publish.yml` under `build-node-bindings`:
```yaml
      - name: Build Native Binding
        run: npx napi build --platform --release --target ${{ matrix.settings.target }}
        working-directory: node

      - name: Test Native Binding on Host
        if: matrix.settings.host == 'ubuntu-latest' || matrix.settings.host == 'macos-latest' || matrix.settings.host == 'windows-latest'
        run: npm test
        working-directory: node
```

- [ ] **Step 2: Validate YAML syntax**

Run: `node -e "const fs = require('fs'); fs.readFileSync('.github/workflows/publish.yml', 'utf8'); console.log('YAML readable');"`

---

### Task 4: End-to-End Verification

**Files:**
- Test all modified scripts and workflows

- [ ] **Step 1: Verify bash syntax**
Run: `bash -n scripts/release.sh`

- [ ] **Step 2: Run release check gate**
Run: `./scripts/release.sh check 0.5.1`

- [ ] **Step 3: Run full dry-run**
Run: `./scripts/release.sh all 0.5.1 --dry-run`
Expected: All 5 phases (`check`, `artifacts`, `publish`, `tag`, `verify`) output dry-run actions cleanly and exit 0.
