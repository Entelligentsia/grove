# PROGRESS — GROVE-S03-T02: Legacy `.grove/explore.json` Migration on Load

## Summary of Changes

Implemented the legacy-migration branch in `GroveConfig::load` so that projects
carrying the old `.grove/explore.json` file (steering key `"mode"`) are
automatically migrated to `.grove/config.json` on the next load, with a
one-time deprecation warning emitted to stderr.

### Changes Made

#### `core/src/explore/config.rs`
- Made `Provider::from_name` and `Steering::from_name` `pub(crate)` so they can
  be called from `config.rs` during migration. (Previously private to the module.)

#### `core/src/config.rs`
- Widened the `use crate::explore::` import to also bring in `Provider` and
  `Steering`.
- Added `pub(crate) const DEPRECATION_WARNING: &str` — a named constant carrying
  the full deprecation message. Using a named const (rather than an inline
  string literal) makes the warning testable without stderr capture.
- Added private `LegacyExploreRaw` struct that mirrors the old `explore.json`
  wire shape: `mode: String` (the old steering key), plus `provider`, `base_url`,
  `model`, `allowed_tools`, `tap`, and `trace_retain` (with serde defaults).
- Added `default_legacy_trace_retain()` helper (required by serde's
  `default = "..."` attribute) that delegates to
  `crate::explore::config::DEFAULT_TRACE_RETAIN`.
- Added `migrate_from_legacy_explore(root: &Path) -> Result<GroveConfig>`:
  reads `explore.json`, maps `mode` → `Steering` via `Steering::from_name`,
  maps `provider` → `Provider` via `Provider::from_name`, constructs
  `ExploreConfig` and `GroveConfig { version:1, mode:McpLlm, ... }`, validates,
  saves `config.json` atomically, emits `DEPRECATION_WARNING` to stderr, and
  returns the config.
- Extended `GroveConfig::load` to a three-branch cascade:
  1. `config.json` exists → existing (read / deserialize / validate) path.
  2. `explore.json` exists → `migrate_from_legacy_explore(root)`.
  3. Neither → actionable `grove init` error (unchanged).
- Added four new tests (AC 5a–5d):
  - `migrate_legacy_explore_writes_config_json` — migration writes `config.json`,
    mode is `McpLlm`, `explore.steering` is `Balanced` (mapped from `mode=balanced`).
  - `second_load_after_migration_reads_config_not_legacy` — second load reads
    `config.json`, returns equal config, `explore.json` mtime is unchanged.
  - `config_json_present_ignores_stale_explore_json` — both files present;
    `config.json` wins, stale `explore.json` is silently ignored.
  - `deprecation_warning_emitted` — asserts `DEPRECATION_WARNING` contains
    `"explore.json"`, `"config.json"`, `"deprecated"`, and `"migrated"`; also
    exercises the migration side-effect path to verify `config.json` is created.

## Test Evidence

```
running 155 tests
test config::tests::bad_version_rejected ... ok
test config::tests::bad_mode_error_names_field_and_legal_values ... ok
test config::tests::explore_section_absent_when_none ... ok
test config::tests::explore_section_present_when_some ... ok
test config::tests::config_json_present_ignores_stale_explore_json ... ok
test config::tests::missing_file_actionable_error ... ok
test config::tests::deprecation_warning_emitted ... ok
test config::tests::serde_round_trip_each_mode ... ok
test config::tests::migrate_legacy_explore_writes_config_json ... ok
test config::tests::steering_key_in_explore_section ... ok
test config::tests::save_load_round_trip_atomic ... ok
test config::tests::second_load_after_migration_reads_config_not_legacy ... ok
...
test result: ok. 155 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s

running 77 tests
...
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 28 tests
...
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

Doc-tests grove_core: 1 passed; 0 failed

Clippy: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.83s (clean, -D warnings)
```

**Total: 261 tests pass. Clippy clean.**

## Files Changed

| File | Change |
|---|---|
| `core/src/config.rs` | `DEPRECATION_WARNING` const; `LegacyExploreRaw` struct; `default_legacy_trace_retain` fn; `migrate_from_legacy_explore` fn; extended `GroveConfig::load`; 4 new tests |
| `core/src/explore/config.rs` | `Provider::from_name` and `Steering::from_name` made `pub(crate)` |

## Acceptance Criteria Status

| # | Criterion | Status |
|---|---|---|
| 1 | `load` migrates when `config.json` absent + `explore.json` present | ✅ Test 5a |
| 2 | Legacy `explore.mode` maps to `explore.steering` | ✅ Test 5a (`balanced` → `Steering::Balanced`) |
| 3 | Deprecation warning emitted to stderr, one-time only | ✅ Test 5d (const verified) + test 5b (idempotency) |
| 4 | Neither file → same actionable error | ✅ Existing `missing_file_actionable_error` test |
| 5 | Four tests (a–d) pass | ✅ All four new tests green |
| 6 | `cargo build` clean, `clippy -D warnings` clean, `cargo test` green | ✅ All pass |
