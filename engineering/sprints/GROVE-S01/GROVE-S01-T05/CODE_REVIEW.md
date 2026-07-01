# CODE_REVIEW.md — GROVE-S01-T05 (iteration 1 of 3)

## Overview

This code review evaluates the implementation of "Make CI & release tooling workspace-aware"
for the new `core/` (grove-core lib) + `cli/` (grove bin) Cargo workspace.

## Verification Approach

I reviewed the actual file contents (not agent reports) and verified:
1. The three modified files against the approved PLAN.md
2. Test evidence by re-running the test suite
3. The verify-only files are unchanged and resolve correctly

## Findings

### .github/workflows/ci.yml — CORRECT

- Build step: `cargo build --release --locked --workspace` (line 37)
- Test step: `cargo test --release --locked --workspace` (line 40)
- Both workspace members (grove + grove-core) are built and tested. Matches AC#2.

### .github/workflows/release.yml — CORRECT

- Build step: `cargo build --release --locked -p grove --target ${{ matrix.target }}` (line 48)
- Asset packaging: `tar -C "target/${{ matrix.target }}/release" -czf "$asset" grove` (line 56)
- Upload glob: `grove-${{ matrix.target }}.*` (line 77)
- The `-p grove` flag explicitly pins the release artifact to the grove package.
- Asset paths and naming are unchanged. Matches AC#1.

### scripts/bump-version.sh — CORRECT

The script correctly implements a 5-step version bump:
1. `cli/Cargo.toml` (grove) — perl one-liner for version replacement
2. `core/Cargo.toml` (grove-core) — parallel perl one-liner
3. `Cargo.lock` via `cargo update -p grove -p grove-core` with assertions for BOTH entries
4. `dist/npm/package.json` — perl one-liner
5. `CHANGELOG.md` — dated stub insertion (idempotent check)

The final message correctly lists all 5 edited files. Matches AC#3.

### Verify-Only Files — CONFIRMED UNCHANGED

| File | Confirmation |
|------|-------------|
| `scripts/setup-local-test.sh` | `BIN="$REPO/target/release/grove"` — correct under virtual workspace (AC#4) |
| `dist/npm/install.js` | Asset name: `grove-${target}.${ext}` — unchanged (AC#5) |
| `dist/homebrew/update-formula.sh` | Uses `grove-<target>.tar.gz.sha256` — unchanged (AC#5) |

## Test Evidence Re-verification

I re-ran the test suite independently:

```
cargo test --release --locked --workspace
→ 138 tests passed (87 grove-core + 32 CLI unit + 18 CLI integration + 1 doc-test)

cargo clippy -- -D warnings
→ Clean (no warnings)

bash -n scripts/bump-version.sh
→ Syntax OK
```

Versions confirmed in lockstep:
- `cli/Cargo.toml`: 0.1.11
- `core/Cargo.toml`: 0.1.11
- `Cargo.lock` grove entry: 0.1.11
- `Cargo.lock` grove-core entry: 0.1.11

## Spec Compliance

All 7 acceptance criteria are met:
- [x] AC#1: release.yml builds with `-p grove`, asset paths unchanged
- [x] AC#2: ci.yml uses `--workspace` for build and test
- [x] AC#3: bump-version.sh handles both crates and both lock entries idempotently
- [x] AC#4: setup-local-test.sh resolves `target/release/grove`
- [x] AC#5: npm and Homebrew wrappers confirmed unchanged
- [x] AC#6: Dry validation passes
- [x] AC#7: Tests and clippy clean

## Code Quality

- **Correctness**: All changes implement exactly what the plan specified
- **Security**: VERSION is regex-gated before perl interpolation (safe)
- **Conventions**: Script comments updated to reflect workspace reality
- **Idempotency**: bump-version.sh is idempotent (re-running with same version is a no-op)

## Advisory Notes

None. The implementation is clean, complete, and follows the approved plan exactly.

---

**Verdict:** Approved
