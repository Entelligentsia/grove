# VALIDATION REPORT — GROVE-S03-T02 (standalone review)

**Task:** Legacy `.grove/explore.json` migration on load (deprecated immediately)  
**Validator:** 🍵 grove QA Engineer  
**Date:** 2026-07-05

---

**Verdict:** Approved

---

## Acceptance Criteria — Verdict per Criterion

### AC1 — `load` migrates when `config.json` absent + `explore.json` present ✅

**Evidence:** `GroveConfig::load` implements a three-branch cascade:
1. `config.json` exists → normal load (T01 path).
2. `explore.json` exists → `migrate_from_legacy_explore(root)`.
3. Neither → bail with actionable error.

Code inspection of `load@210` and test `migrate_legacy_explore_writes_config_json` (5a) confirm the migration branch is triggered when only `explore.json` is present. Test passed in live run.

---

### AC2 — Legacy `explore.mode` maps to `explore.steering` ✅

**Evidence:** `migrate_from_legacy_explore` deserialises into `LegacyExploreRaw`, which has a `mode: String` field (mirroring old wire shape). It calls `Steering::from_name(&raw.mode)` to produce the `steering` value and populates `ExploreConfig { steering, ... }`. Test 5a asserts `explore.steering == Steering::Balanced` when `mode="balanced"` was in the legacy file. Test 5b uses `mode="standard"` — also maps correctly. All test runs confirm no data loss for other fields (`provider`, `base_url`, `model`, `allowed_tools`, `tap`, `trace_retain`).

---

### AC3 — Deprecation warning emitted to stderr, one-time only ✅

**Evidence:**
- `migrate_from_legacy_explore` calls `eprintln!("{DEPRECATION_WARNING}")` as its second-to-last statement, before returning `Ok(config)`.
- `DEPRECATION_WARNING` const is at line ~23 and contains: `"explore.json is deprecated"`, `"migrated to .grove/config.json"`, and `"Please commit the new file and remove .grove/explore.json"`.
- Test 5d verifies the const contains the required strings (`"explore.json"`, `"config.json"`, `"deprecated"`, `"migrated"`).
- **One-time guarantee:** the migration writes `config.json`; subsequent `load` calls enter branch 1 (config.json present) — `eprintln!` is never reached again. Test 5b confirms the second load returns an equal config with `explore.json` mtime unchanged.
- _Note:_ test 5d does not directly capture stderr output (acknowledged tradeoff — cross-platform stdio capture in unit tests is fragile). The implementation emits to stderr via `eprintln!`, which is correct; the test confirms the const is meaningful and the migration branch runs to completion.

---

### AC4 — Neither file → same actionable "run `grove init`" error ✅

**Evidence:** The `load` bail path is:
```rust
bail!("no grove config at {} — run `grove init` to create one, or `grove config` to set it up", path.display())
```
Pre-existing test `missing_file_actionable_error` (line 378) asserts the error message contains `"grove init"` or `"grove config"`. This test passes — 0 regressions.

---

### AC5 — Four tests (a–d) pass ✅

| Test | AC | Result |
|------|----|--------|
| `migrate_legacy_explore_writes_config_json` (5a) | AC1, AC2 | ✅ ok |
| `second_load_after_migration_reads_config_not_legacy` (5b) | AC3 (idempotency) | ✅ ok |
| `config_json_present_ignores_stale_explore_json` (5c) | AC1 (precedence) | ✅ ok |
| `deprecation_warning_emitted` (5d) | AC3 (path exercised) | ✅ ok |

Live run output:
```
test config::tests::migrate_legacy_explore_writes_config_json ... ok
test config::tests::second_load_after_migration_reads_config_not_legacy ... ok
test config::tests::config_json_present_ignores_stale_explore_json ... ok
test config::tests::deprecation_warning_emitted ... ok
```

---

### AC6 — `cargo build` warning-clean, `cargo clippy -- -D warnings` clean, `cargo test` green; files end with newline ✅

**Evidence:**

`cargo clippy -p grove-cst --all-targets -- -D warnings` → clean (no warnings).

Full test suite (`cargo test --release --locked`):
```
test result: ok. 155 passed; 0 failed (unit)
test result: ok.  77 passed; 0 failed (CLI)
test result: ok.  28 passed; 0 failed (integration)
test result: ok.   1 passed; 0 failed (doc-test)
```
Total: **261 tests, 0 failures, 0 regressions.**

Trailing-newline check: both modified files (`core/src/config.rs`, `core/src/explore/config.rs`) end with `0x0a`. ✅

---

## Edge Case and Regression Notes

- **`config.json` + stale `explore.json` coexistence:** test 5c explicitly covers this. `load` short-circuits on `config.json` presence; the stale file is never opened. ✅
- **Malformed `explore.json`:** `migrate_from_legacy_explore` returns `Err` (anyhow context added). This is intentional and documented — the bail path is distinct from the "neither file" steer. Acceptable per approved plan.
- **Migration idempotency:** mtime assertion in test 5b ensures `explore.json` is not written to on second load. ✅
- **`ExploreConfig::legacy_mode_key_rejected` test:** the `ExploreConfig` deserialiser (used in `config.json` branch) correctly rejects a `mode` key, preventing the old shape from being mistakenly accepted through the new code path. This pre-existing test still passes. ✅
- **No deletion of `explore.json`:** confirmed by code inspection — `migrate_from_legacy_explore` never removes the legacy file. The mtime assertion in 5b would detect any unintended write. ✅

---

## Summary

All six acceptance criteria are satisfied. All 261 tests pass with no regressions. Clippy is clean. The implementation is correct and matches the approved plan precisely.
