# Code Review — GROVE-S03-T06 (standalone review)

**Verdict:** Approved

Reviewed `verify_lock` + `LockVerifyStatus` / `LockVerifyEntry` in `core/src/registry.rs` and the curated re-export in `core/src/lib.rs`. Verified independently against the approved PLAN.md — did not rely on PROGRESS.md claims.

## Spec Compliance (verified against PLAN + AC)

1. **`LockVerifyStatus { Match, Mismatch, Missing }`** — present (registry.rs:433), `#[derive(Debug, PartialEq, Eq)]` enabling test equality asserts. ✅
2. **`LockVerifyEntry { lang, expected, actual: Option<String>, status }`** — present (registry.rs:444), fields and doc comments match plan. ✅
3. **`verify_lock(path: &Path) -> Result<Option<Vec<LockVerifyEntry>>>`** — present (registry.rs:465). `Ok(None)` on absent lock; `Ok(Some(vec))` per grammar otherwise. ✅
4. **Wasm path** — `registry_root().join(&name).join("grammar.wasm")`, the same resolution family as `resolve()`. ✅
5. **Status mapping** — `NotFound → Missing`; hash equal → `Match`; hash differ → `Mismatch`; other I/O errors propagated with anyhow context (not silently swallowed). ✅
6. **Hash-format consistency** — recomputes via the existing `sha256()` primitive; `write_lock_for` pins `wasm_sha256()` which delegates to the same `sha256()`, so a fresh lock verifies all-`Match`. Confirmed by the passing `verify_lock_matches_after_write_lock_for` test. ✅
7. **`pub use registry::{LockVerifyEntry, LockVerifyStatus}`** added to lib.rs curated surface (lib.rs:70). ✅
8. **Purely additive** — no signature changes, no lock-format change, no Cargo.toml/new files. ✅

## Review-Plan Advisories — dispositions

- **Malformed-entry handling**: entries missing `name` or `wasm` are `continue`-skipped rather than silently unwrapped — a deliberate, non-panicking choice. Acceptable; the plan-review flagged this as "decide deliberately," and it is decided.
- **registry_root re-eval vs index() OnceLock memoization**: the Match test is registry-root-agnostic (no mid-process env mutation), so the advised divergence risk does not materialize.
- **path is lock-file, not project root**: doc comment on `verify_lock` explicitly states this and directs T07 to pass the resolved `grove.lock` path. ✅

## Independent Verification

- `cargo test --release --locked -p grove-cst verify_lock` → **4 passed, 0 failed** (161 filtered). Confirms the "165 unit tests" claim (161 + 4).
- `cargo clippy -p grove-cst --release --locked` → **clean, no warnings**.
- Tests are authentic and meaningful: Match-after-write round-trip, tampered-hash → Mismatch (asserts `actual.is_some()`), hand-crafted nonexistent lang → Missing (asserts `actual.is_none()`), absent file → `Ok(None)`.

## Advisory Notes (non-blocking)

- The two temp-file tests share `std::process::id()` but use distinct name stems (`match` / `mismatch`), so there is no in-process collision. If future tests are added, prefer a unique-per-test suffix (e.g. append the test name) to keep parallel runs robust.
- `verify_lock` does not validate the lock's top-level `version` field. Fine for a read-only advisory primitive consumed by `grove doctor`; a future forward-compat guard could warn on unknown lock versions.
