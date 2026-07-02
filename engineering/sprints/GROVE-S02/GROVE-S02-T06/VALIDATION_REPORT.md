# VALIDATION REPORT — GROVE-S02-T06 (standalone review)

**Task:** `grove init --as mcp-llm` — target, harness steering (CLAUDE.md/AGENTS.md), .mcp.json  
**Verdict:** Approved

---

## Test Execution

```
cargo test --release --locked
  grove-cst (core):       126 passed, 0 failed
  grove-cst-cli (units):   51 passed, 0 failed
  grove-cst-cli (cli.rs):  22 passed, 0 failed
  grove-cst-cli (other):    1 passed, 0 failed
  Total: 200 passed, 0 failed

cargo clippy --release --locked --all-targets -- -D warnings
  Finished — no warnings
```

---

## Acceptance Criteria Verdicts

### AC1 — `Target::McpLlm` in value enum, `--help`, README ✅ PASS

- `Target::McpLlm` variant added to `cli/src/init.rs` (line 42) with a full doc-comment describing explore-mode, first-run TUI, and re-run/dry-run behaviour.
- `#[derive(ValueEnum)]` on `Target` auto-derives `--as mcp-llm` and the `--help` text entry.
- `README.md` updated in both `--as mcp|skill|both` references to `--as mcp|skill|both|mcp-llm` (lines 68, 180).
- `docs/setup.md` includes a code-block entry and a full "Explore-mode — `grove init --as mcp-llm`" section.

### AC2 — Flow order: provision → TUI → harness ✅ PASS

In `run()`:
1. `provision_project(root, dry_run)` is called first (grammars + `grove.lock`).
2. Dry-run harness print appears before the `provisioned.is_empty()` early-return (plan-review advisory honoured).
3. TUI: `crate::config_tui::run(root, None)` is called when `explore.json` absent (first-run only); skipped on re-runs and dry-run.
4. `write_harness(root, target)` is called after TUI, before the ready block.

### AC3 — Harness files: `.mcp.json`, CLAUDE.md, AGENTS.md, idempotent ✅ PASS

- **`.mcp.json`**: `write_mcp_json_explore()` sets `args: ["serve", "--explore"]` using the read/merge/write pattern; other servers preserved. Unit test `write_mcp_json_explore_registers_with_explore_flag` and `write_mcp_json_explore_preserves_other_servers` confirm.
- **CLAUDE.md**: McpLlm arm in `claude_section()` directs agent to `mcp__grove__explore`; explicitly states individual structural tools are not exposed while provider is healthy; documents automatic fallback to the 7 structural tools.
- **AGENTS.md**: `agents_section()` uses harness-neutral framing; same `<!-- grove:start -->`/`<!-- grove:end -->` sentinels for idempotency. Both files use the shared `CLAUDE_START`/`CLAUDE_END` sentinel constants.
- **Idempotency**: Integration test `mcp_llm_steering_block_idempotency` runs init twice and asserts exactly 1 sentinel pair in each file. Unit tests `write_agents_md_creates_then_updates_idempotently` and `write_claude_md_creates_then_updates_idempotently` confirm in-place update.

### AC4 — Existing targets unchanged ✅ PASS

- `write_harness()` McpLlm branch uses an early `return Ok(wrote)` before the existing `Mcp`/`Skill`/`Both`/`Grammars` paths — those paths are byte-identical.
- `writes_mcp()` matches only `Mcp | Both` (McpLlm excluded), so the generic "ready" block is not triggered for McpLlm.
- All 19 pre-existing unit tests in `init::tests` pass, including `grammars_target_writes_no_harness_files`, `skill_target_writes_steering_but_no_mcp_json`, `mcp_target_writes_mcp_json_and_steering`, `both_target_writes_mcp_json_and_steering`.
- Integration test `init_provisions_and_wires_harness_per_target` (existing) still passes.

### AC5 — Dry-run prints planned actions; non-TTY fails fast ✅ PASS

- Dry-run path (lines 94–98 in `run()`) prints the three planned writes before `provisioned.is_empty()` early-return; TUI is never launched.
- Non-TTY guard: `!std::io::stdout().is_terminal() && !explore_json.exists() && !dry_run` — bails with an explicit init-worded message. Re-runs (explore.json present) bypass the guard so CI is not blocked.
- Integration test `mcp_llm_dry_run_output_shape` asserts: stdout contains "detected", "mcp.json", "CLAUDE.md", "AGENTS.md"; no files written; exit 0.

### AC6 — Integration tests: dry-run shape, idempotency, AGENTS.md create/append ✅ PASS

All three required test cases are present in `cli/tests/cli.rs` and pass:
- `mcp_llm_dry_run_output_shape` — AC5 dry-run shape and no file writes.
- `mcp_llm_steering_block_idempotency` — two consecutive runs produce exactly 1 grove block in CLAUDE.md and AGENTS.md.
- `mcp_llm_agents_md_created_and_appended` — (a) AGENTS.md created when absent; (b) hand-written content preserved, grove block appended once, hand-written content precedes grove block.

### AC7 — Warning-clean, clippy-clean, tests green, no "fastcontext" ✅ PASS

- `cargo test --release --locked`: 200 passed, 0 failed.
- `cargo clippy --release --locked --all-targets -- -D warnings`: clean, no warnings.
- `grep -rn "fastcontext" cli/ core/`: no results.

---

## Edge Cases Verified

| Scenario | Result |
|---|---|
| Non-TTY, explore.json absent, no `--dry-run` | Fails fast with descriptive message (guard line 75) |
| Non-TTY, explore.json absent, `--dry-run` | Prints planned writes, exits 0 (guard skipped) |
| Non-TTY, explore.json **present** | Skips TUI, writes harness normally — CI re-runs unblocked |
| Existing `.mcp.json` with other servers | Other servers preserved; grove entry merged/updated |
| AGENTS.md absent | Created with sentinel block |
| AGENTS.md present (hand-written) | Existing content preserved; grove block appended once |
| Init run twice (idempotency) | Exactly 1 sentinel pair in CLAUDE.md and AGENTS.md |
| `writes_mcp()` for McpLlm | Returns false — generic ready block suppressed, no double-print |

---

## Summary

All 7 acceptance criteria pass with direct test evidence. The implementation is correct against the task prompt ACs and the sprint requirements (SPRINT_REQUIREMENTS.md §1 "New init mode" and §6 "Harness steering"). No regressions; 200 tests green; clippy clean.
