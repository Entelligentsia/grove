# GROVE-S02-T06: `grove init --as mcp-llm` — target, harness steering (CLAUDE.md/AGENTS.md), .mcp.json

**Sprint:** GROVE-S02
**Estimate:** M
**Pipeline:** default

---

## Objective

Wire everything into the one command a user runs: `grove init --as mcp-llm`
provisions grammars, walks the user through the setup TUI, registers the
explore-mode MCP server, and steers the outer agent — so a fresh project goes
from zero to delegated exploration in a single init.

## Acceptance Criteria

1. `Target::McpLlm` added to the `--as` value enum in `cli/src/init.rs`
   (`grove init --as mcp-llm`), documented in `--help` and the README command
   table.
2. The mcp-llm flow, in order: (a) `grove_core::init::provision_project`
   (grammars + `grove.lock`, exactly as other targets); (b) launch the T05 TUI
   to produce `.grove/explore.json` (skipped if `--dry-run`; pre-populated if
   the file exists); (c) write harness files.
3. Harness files written:
   - `.mcp.json` registering grove `serve` **in explore mode** (the T04
     flag/config contract), merging with an existing `.mcp.json` the same way
     current init does.
   - **Idempotent** steering sections in `CLAUDE.md` **and** `AGENTS.md`
     (sentinel-block pattern from the existing `write_claude_md`): direct the
     agent to route file/code exploration through `__explore` instead of
     built-in search/read tools, state that grove's structural tools are not
     individually exposed in this mode, and note the provider-down shutdown
     behavior. Re-running init updates the block in place, never duplicates.
4. Existing targets unchanged: `--as mcp|skill|both|grammars` write exactly the
   files they write today — the current `tests/cli.rs` init assertions pass
   unmodified.
5. `--dry-run` prints the planned actions (provision list + files that would
   be written) without writing or launching the TUI; non-TTY without
   `--dry-run` fails fast with the T05 message.
6. New `tests/cli.rs` cases: mcp-llm dry-run output shape; steering-block
   idempotency (init twice → one block in each of CLAUDE.md / AGENTS.md);
   AGENTS.md created when absent, appended when present.
7. Warning-clean, clippy clean, tests green; no "fastcontext" string.

## Context

Depends on **T04** (the serve-mode registration contract for `.mcp.json`) and
**T05** (the TUI). The steering rationale is VISION §6.4.1 — availability ≠
adoption: registering the server is not enough, the CLAUDE.md/AGENTS.md
directive is what routes a cold agent through `__explore`. AGENTS.md is a new
steering surface for grove (currently only CLAUDE.md is written) — reuse the
same sentinel mechanism, don't fork it.

## Artifacts Involved

- Edited: `cli/src/init.rs` (`Target::McpLlm`, AGENTS.md writer, mcp-llm
  steering block), `cli/src/main.rs` (wiring), `README.md` (command table).
- Verify: `tests/cli.rs` existing init cases; new mcp-llm cases.

## Operational Impact

- **Version bump:** not required (sprint-final).
- **Regeneration:** none for existing projects; new mode is opt-in.
- **Backward compat:** existing init targets byte-identical (criterion 4).
