# VALIDATION REPORT — GROVE-S03-T07 (standalone review)

**Task:** grove doctor — `core::doctor` report + CLI verb (checks, `--json`, exit code)
**Verdict:** Approved

---

## Method

Full test suite executed (`cargo test --release --locked`), `cargo clippy -- -D warnings` run, and each acceptance criterion traced directly to implementation code and test assertions.

---

## Acceptance Criteria Checklist

### AC1 — `core::harness` single source of truth ✅

`core/src/harness.rs` defines `GROVE_START`, `GROVE_END`, `MCP_SERVER_KEY` as `pub const`, and exports `expected_mcp_args(Mode)`, `expected_claude_marker(Mode)`, `agents_md_expected(Mode)`. `cli/src/init.rs` line 16 imports all three constants via `use grove_core::harness::{GROVE_START as CLAUDE_START, GROVE_END as CLAUDE_END, MCP_SERVER_KEY}` — no local duplicate constants remain.

### AC2 — `core::doctor` type surface ✅

`core/src/doctor.rs` defines:
- `pub enum Status { Ok, Warn, Fail, Info }` (verified source)
- `pub struct Check { group, name, status, detail, hint }` (verified source)
- `pub struct Report { mode: Mode, checks: Vec<Check> }` (verified source)
- `pub fn diagnose(root: &Path, force: ModeChoice) -> Report` (verified source) — pure/read-only; no writes inside

### AC3 — Mode resolution via `active_mode` ✅

`cli/src/main.rs` Doctor dispatch converts `--standard`→`ModeChoice::ForceStandard`, `--explore`→`ModeChoice::ForceExplore`, neither→`ModeChoice::None`, then calls `doctor::diagnose(&path, force)`. `diagnose` calls `active_mode(root, force)` internally. Precedence: `--standard` beats `--explore` when both are passed.

### AC4 — Universal checks run for every project ✅

`diagnose` emits exactly the required universal checks in order:
1. `grove_version` (Info)
2. `config_present` (Ok/Fail/Info)
3. `legacy_explore_json` (Warn if drift)
4. `harness_mcp_json` via `check_harness_mcp_json`
5. `harness_claude_md` via `check_harness_claude_md`
6. `harness_agents_md` via `check_harness_agents_md`
7. `harness_serve_surface` via `check_harness_serve_surface`
8. `registry_root` (Ok/Fail)
9. `grammar_cache` (Ok/Warn)
10. `project_languages` (Ok/Warn/Info)
11. `lock_integrity` (Ok/Fail/Warn)

### AC5 — Harness checks → Fail on drift with hint ✅

All three file-level harness check functions use `harness::expected_mcp_args/expected_claude_marker/agents_md_expected`. On any mismatch, `status: Status::Fail` with `hint: Some(format!("grove init --as {}", mode_name(mode)))`. Unit test `harness_matrix_clean_fixtures_all_ok` validates all 5 modes produce Ok on correct fixtures; drift tests `mcp_mode_with_explore_args_in_mcp_json_is_fail`, `mcp_mode_with_explore_marker_in_claude_md_is_fail`, `mcp_llm_mode_without_agents_md_is_warn`, `grammars_mode_with_grove_block_in_claude_md_is_warn` validate specific drift scenarios.

### AC6 — `grove.lock` sha256 verify via T06 `verify_lock` ✅

`lock_integrity` check calls `registry::verify_lock(&lock_path)`: `Ok(None)` (absent) → Warn; per-entry: `Match` → Ok, `Mismatch` → Fail, `Missing` → Warn; overall status is Fail if any Mismatch, else Warn if any Missing, else Ok. Unit test `lock_integrity_absent_lockfile_is_warn` confirms absent → Warn (not Fail).

### AC7 — Explore-mode checks (McpLlm only) ✅

`diagnose` gates `explore_checks()` on `mode == Mode::McpLlm`. `explore_checks` produces: `explore_config_valid`, `provider_reachable`, `model_served`, `allowed_tools_known`, `tap_config`. `HealthError::Unreachable` and `HealthError::ModelMissing` hints surfaced verbatim. Unit tests `explore_config_absent_is_fail` and `provider_unreachable_is_fail` validate edge cases.

### AC8 — `Cmd::Doctor` + `--json` + exit code ✅

`cli/src/main.rs` `Cmd::Doctor { path, explore, standard }` is present. Global `--json` flag at line 24 drives both output paths. Human output groups checks by `group` with ✓/⚠/✗/· icons. JSON output includes `path`, `mode`, `ok`, `checks[]`, `summary` counts. Exit code: `if !report.ok() { std::process::exit(1); }` after both output paths. Integration tests `doctor_universal_clean_exits_zero`, `doctor_json_output_is_valid`, and `doctor_fail_exits_nonzero` (both human and json) confirm both formats.

### AC9 — `Report::ok()` true when only Warn/Info; exits 0 ✅

```rust
pub fn ok(&self) -> bool {
    self.checks.iter().all(|c| !matches!(c.status, Status::Fail))
}
```
Only `Status::Fail` causes `ok()` to return false. Unit test `warn_only_report_exits_zero` builds a report with one Warn and one Info check and asserts `report.ok()` is true.

### AC10 — Build clean, clippy clean, tests green ✅

- `cargo clippy --release -- -D warnings`: **clean** (no warnings or errors)
- `cargo test --release --locked`: **303 tests, 0 failed** (177 core + 92 CLI unit + 33 integration + 1 doc-test)
- Integration test `doctor_help_documents_verb` confirms `--help` includes `doctor`

---

## Edge Cases and Boundary Conditions

| Scenario | Verified By |
|---|---|
| Absent grove.lock → lock_integrity is Warn, not Fail | unit: `lock_integrity_absent_lockfile_is_warn` |
| Warn-only report → exits 0 | unit: `warn_only_report_exits_zero` |
| Drift in `.mcp.json` → Fail with `grove init --as` hint | unit: `mcp_mode_with_explore_args_in_mcp_json_is_fail` |
| McpLlm mode without `AGENTS.md` → Warn | unit: `mcp_llm_mode_without_agents_md_is_warn` |
| Explore checks absent when mode ≠ McpLlm | integration: `doctor_universal_clean_exits_zero` (Mcp fixture, no explore group) |
| `explore_config_valid` absent → Fail + early return | unit: `explore_config_absent_is_fail` |
| Provider unreachable → `model_served` is Info (skipped) | unit: `provider_unreachable_is_fail` |
| Drift exits nonzero under JSON output | integration: `doctor_fail_exits_nonzero` (json branch) |

---

## Regression Check

All 303 pre-existing + new tests pass. No regressions detected.
