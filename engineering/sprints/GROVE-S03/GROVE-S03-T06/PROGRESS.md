# GROVE-S03-T06 — Implementation Progress

## Summary

Added a `verify_lock` function (plus `LockVerifyStatus` enum and `LockVerifyEntry` struct) to `core/src/registry.rs`, giving grove doctor (T07) a read-only primitive to recompute each cached grammar wasm's sha256 and compare it against the lock file.

## Changes Made

### `core/src/registry.rs`
- Added `pub enum LockVerifyStatus { Match, Mismatch, Missing }` — the three-way outcome of comparing a cached wasm's sha256 against the pinned hash.
- Added `pub struct LockVerifyEntry { lang, expected, actual: Option<String>, status }` — one entry per grammar in the lock.
- Added `pub fn verify_lock(path: &Path) -> Result<Option<Vec<LockVerifyEntry>>>`:
  - Returns `Ok(None)` when the lock file is absent (non-error signal).
  - Returns `Ok(Some(entries))` when the lock file parses; each grammar gets an entry.
  - Resolves the wasm path as `registry_root()/<name>/grammar.wasm` (same as `resolve()`).
  - `std::io::ErrorKind::NotFound` → `Missing`; hash match → `Match`; mismatch → `Mismatch`.
  - Other I/O errors are propagated with context.
- Added four unit tests:
  - `verify_lock_returns_none_when_file_absent` — `Ok(None)` for a non-existent path.
  - `verify_lock_matches_after_write_lock_for` — fresh lock verifies all `Match`.
  - `verify_lock_detects_tampered_hash` — mutated `wasm` field yields `Mismatch`.
  - `verify_lock_detects_missing_wasm` — nonexistent lang → `Missing`.

### `core/src/lib.rs`
- Added `pub use registry::{LockVerifyEntry, LockVerifyStatus};` to the curated public surface.

## Test Evidence

```
cargo test --release --locked -p grove-cst

test registry::tests::verify_lock_detects_missing_wasm ... ok
test registry::tests::verify_lock_detects_tampered_hash ... ok
test registry::tests::verify_lock_matches_after_write_lock_for ... ok
test registry::tests::verify_lock_returns_none_when_file_absent ... ok

test result: ok. 165 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
```

All 165 unit tests pass. All 4 new verify_lock tests pass. No regressions.

## Files Changed

- `core/src/registry.rs` — new enum, struct, function, and 4 tests (purely additive)
- `core/src/lib.rs` — new `pub use registry::{LockVerifyEntry, LockVerifyStatus};`
