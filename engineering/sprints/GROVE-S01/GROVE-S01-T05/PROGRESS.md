# PROGRESS — GROVE-S01-T05: Make CI & release tooling workspace-aware

## Summary

All three CI/release tooling files were updated as part of the GROVE-S01 sprint refactor
(alongside T01–T04). This task's implementation phase confirmed the changes are present,
correct, and complete: the build and test workflows are workspace-aware, the release
workflow pins the binary package explicitly, and the version-bump script covers both
workspace members in one idempotent shot. Verify-only files were confirmed unchanged
and correct. All tests pass.

## Changes Made

### .github/workflows/ci.yml (AC#2)

Build step changed from:
```
cargo build --release --locked
```
to:
```
cargo build --release --locked --workspace
```

Test step changed from:
```
cargo test --release --locked
```
to:
```
cargo test --release --locked --workspace
```

Both workspace members (`grove` and `grove-core`) are now built and tested by CI.

### .github/workflows/release.yml (AC#1)

Build step changed from:
```
cargo build --release --locked --target ${{ matrix.target }}
```
to:
```
cargo build --release --locked -p grove --target ${{ matrix.target }}
```

The `-p grove` flag pins the release artifact to the `grove` binary package. Asset
packaging paths (`target/<target>/release/grove[.exe]`) and the upload glob
(`grove-<target>.*`) are byte-for-byte identical — zero end-user impact.

### scripts/bump-version.sh (AC#3)

Extended from a 4-step to a 5-step script:
- Step 1: `cli/Cargo.toml` (grove) — unchanged
- Step 2 (new): `core/Cargo.toml` (grove-core) — perl one-liner mirrors the cli step
- Step 3: `cargo update -p grove -p grove-core` (was `-p grove` only)
- Step 3 lock assertions: asserts both `grove` and `grove-core` Cargo.lock entries equal the target version
- Step 4: `dist/npm/package.json` — unchanged
- Step 5: `CHANGELOG.md` stub — unchanged
- Summary message updated: `Done. Edited: cli/Cargo.toml, core/Cargo.toml, Cargo.lock, dist/npm/package.json, CHANGELOG.md.`

## Verify-Only Confirmations

### scripts/setup-local-test.sh (AC#4)

`cargo build --release` at workspace root still produces `target/release/grove` (confirmed:
`BIN="$REPO/target/release/grove"` at line 28). Under a virtual workspace, the root
`cargo build` delegates to all members; the grove binary always lands at
`target/release/grove`. No change needed.

### dist/npm/install.js (AC#5)

Downloads `grove-${target}.${ext}` and installs `grove`/`grove.exe` (lines 68–94).
Asset names are derived from the release.yml upload glob, which is unchanged. No
change needed.

### dist/npm/package.json (AC#5)

`"bin": {"grove": "bin/grove.js"}` — no layout dependency on workspace structure.
Version touched only by bump script. No change needed.

### dist/homebrew/update-formula.sh + grove.rb (AC#5)

Pulls `grove-<target>.tar.gz.sha256` by name from the GitHub Release (line 18:
`curl -fsSL "$base/grove-$1.tar.gz.sha256"`). Asset names are unchanged. No
change needed.

## Test Evidence

### cargo build --release --locked -p grove (AC#6)

```
Finished `release` profile [optimized] target(s) in 0.08s
```
Binary confirmed at `target/release/grove` (12 677 704 bytes).

### cargo test --release --locked --workspace (AC#2, AC#6, AC#7)

```
running 32 tests
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

running 87 tests
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s

Doc-tests grove_core
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

Total: 138 tests, all passing (32 CLI/mcp unit + 18 CLI integration + 87 grove-core unit + 1 doc-test).

### cargo clippy -- -D warnings (AC#7)

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
```
No warnings. Clean.

### scripts/bump-version.sh 0.1.11 (idempotency, AC#3)

```
== 1/5  cli/Cargo.toml (grove) -> 0.1.11
version = "0.1.11"

== 2/5  core/Cargo.toml (grove-core) -> 0.1.11
version = "0.1.11"

== 3/5  Cargo.lock (grove + grove-core entries)
Cargo.lock grove version = "0.1.11"
Cargo.lock grove-core version = "0.1.11"

== 4/5  dist/npm/package.json -> 0.1.11
  "version": "0.1.11",

== 5/5  CHANGELOG.md
CHANGELOG already has a [0.1.11] section — leaving it.
```

Idempotent: `git diff` shows no spurious changes to any source file.

### YAML syntax validation

```
OK: .github/workflows/ci.yml
OK: .github/workflows/release.yml
```

### bash -n scripts/bump-version.sh

```
syntax OK
```

## Acceptance Criteria Status

- [x] AC#1: `release.yml` builds with `-p grove` for all 5 targets; asset path and `grove-<target>.*` upload glob unchanged.
- [x] AC#2: `ci.yml` runs `cargo build --release --locked --workspace` and `cargo test --release --locked --workspace`.
- [x] AC#3: `bump-version.sh` moves `core/Cargo.toml`, `cli/Cargo.toml`, both `Cargo.lock` entries (`grove` + `grove-core`), and `dist/npm/package.json` to the same version in one idempotent run.
- [x] AC#4: `scripts/setup-local-test.sh` still resolves `target/release/grove` under the workspace.
- [x] AC#5: npm install wrapper and `dist/homebrew/update-formula.sh` confirmed to resolve/package the `grove` binary unchanged.
- [x] AC#6: Dry validation passes — `cargo build --release --locked -p grove` locally + release.yml asset-name review.
- [x] AC#7: `cargo test --release --locked --workspace` and `cargo clippy -- -D warnings` are green.

## Files Changed

| File | Change |
|---|---|
| `.github/workflows/ci.yml` | Added `--workspace` to build and test steps |
| `.github/workflows/release.yml` | Added `-p grove` to build step |
| `scripts/bump-version.sh` | Extended to 5-step, adds core/Cargo.toml bump + grove-core lock assertion |
