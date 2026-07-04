# GROVE-S03-T06 Validation Report (standalone review)

**Task:** grove.lock wasm sha256 verify primitive (core)
**Verdict:** Approved

---

## Acceptance Criteria Verdict

### AC#1 — `verify_lock(path)` returns `Ok(None)` when the lock file is absent
**PASS**

Test `verify_lock_returns_none_when_file_absent` constructs a path guaranteed not to exist and asserts `result.is_none()`. Test passes:
```
test registry::tests::verify_lock_returns_none_when_file_absent ... ok
```
Code path confirmed: `if !path.exists() { return Ok(None); }` at the top of `verify_lock`.

---

### AC#2 — `verify_lock(path)` returns per-language `LockVerifyEntry` with `Match`, `Mismatch`, or `Missing` status
**PASS**

All three status arms are covered by distinct tests:
- `verify_lock_matches_after_write_lock_for` → `Match` — asserts `entries[0].status == LockVerifyStatus::Match` and `actual.is_some()`.
- `verify_lock_detects_tampered_hash` → `Mismatch` — overwrites the `wasm` JSON field with a bogus hash; asserts `status == Mismatch` and `actual.is_some()` (wasm still on disk).
- `verify_lock_detects_missing_wasm` → `Missing` — hand-crafts a lock referencing `grove_nonexistent_lang_xyz`; asserts `status == Missing` and `actual.is_none()`.

All four tests pass:
```
test registry::tests::verify_lock_returns_none_when_file_absent ... ok
test registry::tests::verify_lock_detects_missing_wasm ... ok
test registry::tests::verify_lock_matches_after_write_lock_for ... ok
test registry::tests::verify_lock_detects_tampered_hash ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 161 filtered out
```

`LockVerifyStatus` derives `PartialEq + Eq` — assertions using `assert_eq!` are structurally sound, not vacuous.

---

### AC#3 — Reuses `sha256()` primitive ensuring a freshly written lock verifies clean
**PASS**

Inspected source directly: `verify_lock` calls `sha256(&bytes)` at line 493; `write_lock_for` calls `wasm_sha256(self.wasm)` which delegates to the same `sha256` function. The round-trip test (`verify_lock_matches_after_write_lock_for`) confirms this end-to-end: write then verify yields all-Match without any hash reformatting step.

---

### AC#4 — Four unit tests covering all status variants plus the absent-lock case
**PASS**

Exactly four tests are present in `registry::tests`:
1. `verify_lock_returns_none_when_file_absent` — absent lock (Ok(None))
2. `verify_lock_matches_after_write_lock_for` — Match
3. `verify_lock_detects_tampered_hash` — Mismatch
4. `verify_lock_detects_missing_wasm` — Missing

All four pass. All assertions are specific (checking `status`, `actual`, `lang`, `entries.len()`). No vacuous assertions observed.

---

### AC#5 — `cargo build` warning-clean, `cargo clippy -- -D warnings` clean, `cargo test` green; all files end with a newline
**PASS**

- **cargo clippy --release --locked -- -D warnings**: zero warnings, clean exit.
- **cargo test --release --locked**: 165 unit tests + 92 + 29 + 1 doc-tests = 287 total, 0 failed.
- **File endings**: `tail -c 1 core/src/registry.rs | xxd` → `0x0a`; same for `core/src/lib.rs`.

---

## Additional Checks

### Public surface (lib.rs re-export)
`grep "pub use registry" core/src/lib.rs` → line 70: `pub use registry::{LockVerifyEntry, LockVerifyStatus};` ✅

### Struct fields and types
`LockVerifyEntry` has: `lang: String`, `expected: String`, `actual: Option<String>`, `status: LockVerifyStatus` — all public, matches plan specification. ✅

### Error propagation
NotFound I/O error → `Missing` status (no panic, no silent swallow). Other I/O errors propagated via `anyhow` with `context(...)`. Malformed entries (missing `name`/`wasm` fields) are `continue`-skipped — deliberate non-panicking choice per plan-review advisory. ✅

### Purely additive, no breaking changes
No existing signatures changed. No Cargo.toml changes. No lock-format changes. ✅

### Regression
Full 287-test suite passes without failures. No regressions detected. ✅

---

## Summary

All five acceptance criteria are met with direct evidence. The implementation is correct, the tests are authentic (specific assertions, meaningful scenarios), the public surface is correctly exported, and the full test suite remains green. No issues found.
