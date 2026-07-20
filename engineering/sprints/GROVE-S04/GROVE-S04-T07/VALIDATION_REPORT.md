# Validation Report — GROVE-S04-T07 (standalone review)

**Task:** `doctor` re-keying — explore checks keyed on `grove-explore` registration, not mode  
**Verdict:** Approved

## Validation Summary

All acceptance criteria are met by the implementation. The test suite, lint, and newline checks pass. Boundary conditions are covered by new unit/integration tests; no regressions were observed in related functionality.

## Acceptance Criteria Checklist

### AC1 — Explore check group runs only on `grove-explore` registration, not declared mode
**Status:** Pass

- `core/src/doctor.rs::diagnose` now computes `has_explore_reg` via `has_explore_registration`, which scans every selected harness config (Claude `.mcp.json`, Cursor `.cursor/mcp.json`, VS Code, Codex global TOML, Gemini, Windsurf) for a `grove-explore` server entry.
- The explore check group is gated by `if has_explore_reg { checks.extend(explore_checks(...)); }` rather than `if mode == Mode::McpLlm`.
- `check_harness_serve_surface` now reports "Explore"/"Standard" based on `has_explore_reg`, not mode inference.
- Tests pin both sides of the gate:
  - `mcp_llm_cursor_only_explore_registration_triggers_explore_group` — non-Claude Cursor-only explore registration triggers explore checks even when declared mode is not the driver.
  - `mcp_llm_without_explore_registration_skips_explore_group` — declared `mcp-llm` with no `grove-explore` registration skips the explore group.
  - `mcp_mode_with_cursor_explore_registration_is_fail` — a non-`mcp-llm` mode with an unexpected `grove-explore` registration is rejected.

*Note:* This is the approved harness-agnostic widening of the literal task-prompt AC text ("project's `.mcp.json`") to any selected harness, as documented in `PLAN_REVIEW.md` and `CODE_REVIEW.md`.

### AC2 — Structural checks, `--json` shape, and exit-code gate preserved
**Status:** Pass

- The universal checks (`config_present`, `legacy_explore_json`, `registry_root`, `grammar_cache`, `project_languages`, `lock_integrity`) are unchanged in `diagnose`.
- `cli/src/main.rs` continues to emit the same `ok` + `checks` JSON object and exits `0` only when `report.ok()` is true.
- Integration tests verify this: `doctor_json_output_is_valid` asserts the `ok` field and `checks` array; `doctor_fail_exits_nonzero` asserts non-zero exit and `ok:false` for harness drift in both human and `--json` modes.

### AC3 — Harness-consistency / drift matrix understands the two-registration and stale-layout cases
**Status:** Pass

- `check_harness_mcp_json` (Claude) and `check_harness_registration` (all other harnesses) validate both the structural `grove` entry and the `grove-explore` entry against the declared mode.
- Expected registrations come from `harness::expected_mcp_args(mode)` and `harness::expected_explore_args(mode)`:
  - `mcp-llm` expects both `grove` and `grove-explore`.
  - Other modes expect only `grove` (or no entry, depending on mode) and no `grove-explore`.
- `has_stale_explore_arg` detects a pre-split single-registration layout where `grove` still carries the removed `--explore` argument and short-circuits to a Fail with hint `grove init --as mcp-llm`.
- Boundary tests cover:
  - `mcp_llm_missing_explore_entry_is_fail` — `mcp-llm` without `grove-explore` fails with the `mcp-llm` re-init hint.
  - `mcp_mode_with_explore_args_in_mcp_json_is_fail` — unexpected `--explore` in a non-`mcp-llm` `.mcp.json` fails.
  - `mcp_llm_stale_layout_single_registration_is_fail` — pre-split stale layout is flagged.
  - `multi_harness_config_checks_each_registration` and `multi_harness_missing_cursor_registration_is_fail` — multiple selected harnesses are each validated.

### AC4 — `grove doctor` no longer accepts `--explore` or `--standard`; `--json` and exit-code gate preserved
**Status:** Pass

- The `Cmd::Doctor` variant in `cli/src/main.rs` no longer has `explore`/`standard` fields, and the `ModeChoice::ForceExplore`/`ForceStandard` plumbing is removed.
- `ModeChoice` is conservatively pruned to a single `None` variant; `active_mode` is retained for API compatibility.
- `doctor_help_documents_verb` asserts that `--explore` and `--standard` are absent from `grove doctor --help`.

### AC5 — `cargo test --release --locked` passes
**Status:** Pass

```
running 139 tests ... ok
running 65 tests ... ok
running 43 tests ... ok
running 42 tests ... ok
running 54 tests ... ok
Doc-tests grove_core ... ok
Doc-tests grove_explore_core ... ok
```
All 343 tests pass.

Doctor-specific tests: 15 unit tests in `core/src/doctor.rs` and 4 integration tests in `cli/tests/cli.rs` all pass.

### AC6 — `cargo clippy --all-targets --workspace --locked -- -D warnings` passes
**Status:** Pass

```
    Finished `dev` profile [unoptimized] target(s) in 0.09s
```
No warnings.

### AC7 — All modified files end with a newline
**Status:** Pass

Verified with `tail -c 1`:
- `core/src/doctor.rs`: newline OK
- `core/src/config.rs`: newline OK
- `cli/src/main.rs`: newline OK
- `cli/tests/cli.rs`: newline OK

## Edge-Case Observations

- **Malformed harness configs:** `read_server_args` swallows parse errors to `None`, so a malformed config is treated as "no registration present." This is conservative and safe.
- **Mode-blind stale hint:** As noted in `CODE_REVIEW.md`, the stale-layout hint always says `grove init --as mcp-llm`, even if the user downgraded from `mcp-llm` to another mode. This matches the approved plan and is a rare migration edge case.
- **No CLI force flags remain:** `ModeChoice` is now a single-variant enum. The conservative API-preservation approach avoids touching `core/src/lib.rs` and `cli/src/init.rs` as planned.

## Regression Check

- Full workspace test suite passes.
- Clippy is clean at `-D warnings`.
- No existing doctor tests were removed; force-variant unit tests in `core/src/config.rs` were removed as expected because the variants no longer exist.
