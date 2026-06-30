# PROGRESS: GROVE-S01-T01 — Establish Cargo workspace; relocate crate into cli/

## Summary

Converted the single-crate grove repository to a Cargo virtual workspace. The root
`Cargo.toml` now declares only a `[workspace]` section with `members = ["cli"]` and
`resolver = "2"`. All package metadata, dependencies, and the binary target were
moved into the new `cli/` sub-crate. Two pre-existing clippy lints were fixed as
part of the move (they were suppressed by the prior non-clippy CI but surface under
`-D warnings`).

## Changes Made

### Step 1 — Workspace root manifest
Replaced `Cargo.toml` with a virtual workspace manifest:
- `[workspace]` with `members = ["cli"]`, `resolver = "2"`
- `[profile.release]` moved here from the sub-crate (Cargo ignores profile sections
  in non-root packages; moving it silences the warning and preserves the `lto`/`strip`
  settings)

### Step 2 — Sub-crate manifest
Created `cli/Cargo.toml` carrying over:
- Full `[package]` block (name, version, edition, description, license, repository,
  homepage, keywords, categories)
- `readme = "../README.md"` (adjusted path so `cargo publish` can find it)
- `[[bin]]` block (`name = "grove"`, `path = "src/main.rs"`)
- All `[dependencies]` unchanged

### Step 3 — Source relocation
- `src/` (8 files) moved to `cli/src/`
- `tests/` (1 file) moved to `cli/tests/`

### Step 4 — DEV_REGISTRY path fix
`cli/tests/cli.rs` line 11:
```
- const DEV_REGISTRY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/registry");
+ const DEV_REGISTRY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../registry");
```
`CARGO_MANIFEST_DIR` now resolves to `<repo>/cli`; the `registry/` dev stub remains
at the repo root.

### Step 5 — bump-version.sh update
Updated `scripts/bump-version.sh` step 1/4: `perl` invocation and `grep` now target
`cli/Cargo.toml` instead of `Cargo.toml`.

### Step 6 — Clippy fixes (pre-existing lints)
Two warnings that were present in the original code but become errors under
`-D warnings`:
- `cli/src/mcp.rs:344`: `(0 | 1 | 2)` → `0..=2` (manual_range_patterns)
- `cli/src/ops.rs:453`: `for (_, s) in syms.iter().enumerate()` → `for s in syms.iter()` (unused_enumerate_index)

## Test Evidence

```
$ cargo build --release --locked
   Compiling grove v0.1.11 (/…/cli)
    Finished `release` profile [optimized] target(s) in 32.29s

$ cargo test --release --locked
     Running unittests src/main.rs (…)
running 117 tests
... (all pass)
test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s

     Running tests/cli.rs (…)
running 17 tests
... (all pass)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

$ cargo clippy -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s
```

Total: 134 tests passed, 0 failed. Clippy clean.

## Files Changed

| File | Action |
|------|--------|
| `Cargo.toml` | Replaced with virtual workspace manifest |
| `cli/Cargo.toml` | Created with package metadata + deps |
| `cli/src/engine.rs` | Moved from `src/engine.rs` |
| `cli/src/fetch.rs` | Moved from `src/fetch.rs` |
| `cli/src/ingest.rs` | Moved from `src/ingest.rs` |
| `cli/src/init.rs` | Moved from `src/init.rs` |
| `cli/src/main.rs` | Moved from `src/main.rs` |
| `cli/src/mcp.rs` | Moved from `src/mcp.rs`; clippy fix on line 344 |
| `cli/src/ops.rs` | Moved from `src/ops.rs`; clippy fix on line 453 |
| `cli/src/registry.rs` | Moved from `src/registry.rs` |
| `cli/tests/cli.rs` | Moved from `tests/cli.rs`; DEV_REGISTRY path fixed |
| `scripts/bump-version.sh` | Updated step 1/4 to target `cli/Cargo.toml` |
