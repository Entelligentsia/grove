# CODE_REVIEW — GROVE-S04-T08 (iteration 1 of 3)

**Verdict:** Approved

## Review Scope

Re-review of the revision pass that addresses the version-corruption finding from the prior code review. The prior review found that `bump-version.sh 9.9.9` was run against the real working tree without proper restoration, leaving all workspace versions at `9.9.9`.

## Verification Results

### 1. Version Files — All at 0.4.1

Independently verified (not from PROGRESS.md):

| File | Value | Status |
|------|-------|--------|
| `core/Cargo.toml` | `version = "0.4.1"` | OK |
| `cli/Cargo.toml` | `version = "0.4.1"` | OK |
| `explore/Cargo.toml` | `version = "0.4.1"` | OK |
| `grove-explore/Cargo.toml` | `version = "0.4.1"` | OK |
| `Cargo.lock` (4 crates) | all `version = "0.4.1"` | OK |
| `dist/npm/package.json` | `"version": "0.4.1"` | OK |
| `CHANGELOG.md` | no `9.9.9` section | OK |

The `grep -c "9.9.9"` matches in `Cargo.lock` are checksum hashes, not version strings.

### 2. Binary --version Smoke Check

```
$ ./target/release/grove --version
grove 0.4.1

$ ./target/release/grove-explore --version
grove-explore 0.4.1
```

Both binaries report correct versions. The `grove-explore/src/main.rs` line 34 includes the `version` attribute in `#[command]`.

### 3. Test Suite

```
cargo test --release --locked --workspace
```

All tests pass: 42 (core) + 54 (explore) + 1 (doc-test) = 97 passed, 0 failed.

### 4. Lint (CI-equivalent)

```
cargo clippy --all-targets --workspace --locked -- -D warnings
```

Clean — no warnings.

### 5. RELEASING.md — Bump Script Testing Section

The new "Testing the bump script" subsection is present and comprehensive:
- Documents the run→verify→restore→checksum protocol
- Explains why `git checkout HEAD --` plus targeted perl edit for package.json
- Warns against testing without a restore plan

### 6. Dual-Binary Release Train (preserved from prior implementation)

Verified all packaging files support both binaries:

| Component | Status |
|-----------|--------|
| `.github/workflows/release.yml` | Builds both with `-p grove-cst-cli -p grove-explore-bin`, tars/zips both, 4-crate publish order |
| `dist/npm/bin/grove.js` | Shim exists |
| `dist/npm/bin/grove-explore.js` | Shim exists (new) |
| `dist/npm/install.js` | Loops over both binaries |
| `dist/homebrew/grove.rb` | Installs and tests both |
| `install.sh` | Conditional guard for grove-explore |

### 7. PROGRESS.md Evidence

The PROGRESS.md now contains fresh, non-stale evidence:
- Version verification with actual grep output
- Forced-rebuild compilation output showing 0.4.1
- Fresh --version output (matching independent verification above)
- Bump-script run→verify→restore→checksum cycle with before/after checksums
- All syntax checks passing

## Prior Code Review Findings — Resolution

| Finding | Resolution |
|---------|------------|
| Version files corrupted with 9.9.9 | All restored to 0.4.1; verified independently |
| PROGRESS.md evidence stale (showed 0.4.1 but binaries were 9.9.9) | Fresh evidence with matching independent verification |
| No documented safe bump-script testing procedure | RELEASING.md now includes "Testing the bump script" section |

## Advisory Notes

1. The Cargo.lock "9.9.9" grep matches (4 hits) are checksum substrings, not version numbers — this is expected and not a concern.

2. The bump-script test protocol requires manual attention to `dist/npm/package.json` if it has uncommitted changes beyond the version field. The perl one-liner in RELEASING.md handles this correctly.
