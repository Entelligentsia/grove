# PROGRESS — GROVE-S03-T07: grove doctor

## Summary

Implemented `grove doctor` end-to-end per the approved plan:

1. **`core/src/harness.rs`** — new module: single source of truth for all harness
   sentinel constants (`GROVE_START`, `GROVE_END`, `MCP_SERVER_KEY`) and per-mode
   query functions (`expected_mcp_args`, `expected_claude_marker`,
   `agents_md_expected`). Eliminates the constant-ownership blocker.

2. **`core/src/lib.rs`** — added `pub mod harness;` and `pub mod doctor;`.

3. **`cli/src/init.rs`** — removed the three private `const` definitions
   (`CLAUDE_START`, `CLAUDE_END`, `MCP_SERVER_KEY`) and replaced them with
   `use grove_core::harness::{GROVE_START as CLAUDE_START, GROVE_END as CLAUDE_END, MCP_SERVER_KEY}`.
   All downstream uses unchanged (alias preserves the names). The sentinel strings
   now live in exactly one place in the codebase.

4. **`core/src/doctor.rs`** — new module implementing:
   - `Status { Ok, Warn, Fail, Info }` — severity enum
   - `Check { group, name, status, detail, hint }` — single diagnostic result
   - `Report { mode, checks }` + `Report::ok()` — full report; `ok()` is true
     when no `Fail` is present (warnings are pass-grade per AC 9)
   - `diagnose(root: &Path, force: ModeChoice) -> Report` — pure read-only;
     runs 11 universal checks and (when mode=McpLlm) 5 explore-mode checks
   - All harness sub-checks use `harness::expected_mcp_args/expected_claude_marker/agents_md_expected`
   - Lock integrity uses `registry::verify_lock`
   - Explore health checks use `health_probe` + `HealthError` variants

5. **`cli/src/main.rs`** — added `Cmd::Doctor { path, explore, standard }` variant
   and dispatch arm with:
   - `--explore`/`--standard` → `ModeChoice` conversion matching `Cmd::Serve`
   - Human output: grouped `✓/⚠/✗/·` table per group, failure count at end
   - JSON output (`--json`): `{ path, mode, ok, checks[], summary:{ok,warn,fail,info} }`
   - `std::process::exit(1)` when `!report.ok()`, under both output formats

6. **`cli/tests/cli.rs`** — added 4 integration tests:
   - `doctor_help_documents_verb` — `grove doctor --help` exits 0 and mentions "doctor"
   - `doctor_universal_clean_exits_zero` — clean mcp fixture exits 0
   - `doctor_json_output_is_valid` — `--json` output parses and has `ok` + `checks`
   - `doctor_fail_exits_nonzero` — harness drift exits 1 under both output modes

## Test Evidence

```
cargo test --release --locked 2>&1

running 177 tests
... (harness + doctor + all existing tests)
test doctor::tests::warn_only_report_exits_zero ... ok
test doctor::tests::lock_integrity_absent_lockfile_is_warn ... ok
test doctor::tests::explore_config_absent_is_fail ... ok
test doctor::tests::mcp_mode_with_explore_marker_in_claude_md_is_fail ... ok
test doctor::tests::grammars_mode_with_grove_block_in_claude_md_is_warn ... ok
test doctor::tests::mcp_mode_with_explore_args_in_mcp_json_is_fail ... ok
test doctor::tests::mcp_llm_mode_without_agents_md_is_warn ... ok
test doctor::tests::provider_unreachable_is_fail ... ok
test doctor::tests::harness_matrix_clean_fixtures_all_ok ... ok
test harness::tests::agents_md_expected_only_mcp_llm ... ok
test harness::tests::expected_claude_marker_coverage ... ok
test harness::tests::expected_mcp_args_coverage ... ok
test result: ok. 177 passed; 0 failed; 0 ignored

running 92 tests
... (all existing cli unit tests pass)
test result: ok. 92 passed; 0 failed; 0 ignored

running 33 tests
test doctor_universal_clean_exits_zero ... ok
test doctor_json_output_is_valid ... ok
test doctor_help_documents_verb ... ok
test doctor_fail_exits_nonzero ... ok
... (all 29 prior integration tests pass)
test result: ok. 33 passed; 0 failed; 0 ignored

Doc-tests grove_core
test result: ok. 1 passed; 0 failed; 0 ignored
```

**Total: 303 tests, 0 failures.**

`cargo clippy -- -D warnings` clean.

## Acceptance Criteria Coverage

| AC | Status |
|----|--------|
| 1. `core::harness` defines constants + query fns; `init.rs` imports them | ✅ |
| 2. `core::doctor` defines `Status`, `Check`, `Report`, `diagnose` | ✅ |
| 3. Mode resolution via `active_mode(root, force)` | ✅ |
| 4. Universal checks run for every project | ✅ |
| 5. Harness checks → Fail on drift with `grove init --as <mode>` hint | ✅ |
| 6. `grove.lock` sha256 verify via T06 `verify_lock` | ✅ |
| 7. Explore-mode checks (McpLlm only) | ✅ |
| 8. `Cmd::Doctor { path, explore, standard }` + `--json` + exit code | ✅ |
| 9. `Report::ok()` true when only Warn/Info; exits 0 | ✅ (unit test) |
| 10. `cargo clippy` clean; `cargo test` green | ✅ |

## Files Changed

- `core/src/harness.rs` (new)
- `core/src/doctor.rs` (new)
- `core/src/lib.rs` (add `pub mod harness;`, `pub mod doctor;`)
- `cli/src/init.rs` (migrate sentinel consts to `grove_core::harness` imports)
- `cli/src/main.rs` (add `Cmd::Doctor` + dispatch + helpers)
- `cli/tests/cli.rs` (add 4 doctor integration tests)
