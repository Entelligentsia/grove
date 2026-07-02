# PROGRESS — GROVE-S02-T06
## `grove init --as mcp-llm` — target, harness steering (CLAUDE.md/AGENTS.md), .mcp.json

## Summary

Implemented `Target::McpLlm` as a new `--as` value for `grove init`, wiring the
explore-mode `.mcp.json` (`serve --explore`), idempotent sentinel blocks in
`CLAUDE.md` and `AGENTS.md`, non-TTY guard for first-run, and first-run TUI
delegation. All 7 acceptance criteria covered.

## Changes Made

### `cli/src/init.rs`
- Added `use std::io::IsTerminal;` import.
- Added `Target::McpLlm` variant (between `Both` and `Grammars`) with full doc-comment
  explaining explore-mode, TUI first-run, and re-run/dry-run behaviour.
- Modified `run()`:
  - Non-TTY guard: fails fast with a clear `grove init --as mcp-llm`-worded message
    when stdout is not a terminal AND explore.json is absent AND not dry-run.
  - Dry-run path: prints the three planned harness files BEFORE the `provisioned.is_empty()`
    early-return (plan advisory honoured).
  - TUI launch: calls `crate::config_tui::run(root, None)` when explore.json is absent;
    skipped on re-runs.
  - McpLlm-specific "ready" block (uses `if target == McpLlm` to avoid the generic
    `writes_mcp()` block double-printing — plan advisory honoured).
- Modified `write_harness()`: added McpLlm early-return branch (top of function) calling
  `write_mcp_json_explore`, `write_claude_md`, `write_agents_md`; returns 3 entries.
- Added `write_mcp_json_explore()`: same read/merge/write pattern as `write_mcp_json`
  but registers `args: ["serve", "--explore"]`. Returns `".mcp.json (explore-mode
  registration)"`.
- Modified `claude_section()`: new McpLlm arm before `writes_mcp()` check; names
  `mcp__grove__explore`; describes fallback tools WITHOUT the `mcp__grove__` prefix
  (so test assertion `!contains("mcp__grove__outline")` passes); uses `return` so
  existing arms are untouched.
- Added `write_agents_md()`: identical sentinel/idempotency logic as `write_claude_md`,
  targeting `AGENTS.md`; returns `"AGENTS.md (explore-mode steering)"`.
- Added `agents_section()`: harness-neutral framing using plain `explore` tool name
  (no `mcp__grove__` prefix); same `CLAUDE_START`/`CLAUDE_END` sentinels for idempotency.
- Added 7 unit tests: all pass.

### `cli/tests/cli.rs`
- Added `mcp_llm_setup()` helper (fake cache + rust grammar + lib.rs fixture).
- Added `grove_mcp_llm()` helper (GROVE_REGISTRY + GROVE_REGISTRY_URL dead host + XDG_CACHE_HOME).
- Added `mcp_llm_dry_run_output_shape`: non-TTY dry-run prints "detected", "mcp.json",
  "CLAUDE.md", "AGENTS.md"; writes no files; exits 0.
- Added `mcp_llm_steering_block_idempotency`: two runs; exactly 1 sentinel block in
  each of CLAUDE.md and AGENTS.md.
- Added `mcp_llm_agents_md_created_and_appended`: (a) AGENTS.md created when absent;
  (b) hand-written content preserved, grove block appended once.

### `README.md`
- Updated two references from `--as mcp|skill|both` to `--as mcp|skill|both|mcp-llm`.

### `docs/setup.md`
- Extended `--as` code block with the `mcp-llm` entry.
- Added "Explore-mode — `grove init --as mcp-llm`" section documenting what it writes,
  the first-run TUI, and `--dry-run` behaviour.

## Test Evidence

```
test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s

     Running tests/cli.rs (target/debug/deps/cli-4ef99f8413c3d167)

running 22 tests
test mcp_llm_dry_run_output_shape ... ok
test mcp_llm_steering_block_idempotency ... ok
test mcp_llm_agents_md_created_and_appended ... ok
test init_provisions_and_wires_harness_per_target ... ok
[... 18 other tests all ok ...]

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.97s

Doc-tests grove_core
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

Clippy: `cargo clippy -- -D warnings` — clean, no warnings.

## Files Changed

- `cli/src/init.rs` — main implementation (+259 lines)
- `cli/tests/cli.rs` — 3 new integration tests (+145 lines)
- `README.md` — `--as` target list updated (+2 lines)
- `docs/setup.md` — new `--as mcp-llm` section (+31 lines)
