# PLAN REVIEW — GROVE-S02-T06 (standalone review)

**Verdict:** Approved

## Summary

The plan to add `Target::McpLlm` (`grove init --as mcp-llm`) — wiring explore-mode
`.mcp.json`, idempotent `CLAUDE.md`/`AGENTS.md` sentinel steering, and a first-run
TUI launch — is feasible, correctly grounded in the actual `cli/src/init.rs`
structure, and covers all seven acceptance criteria. Every structural assumption
was independently verified against source.

## Verification performed (against actual code, not the plan's claims)

1. **`Target` enum / clap** — `Target` derives `ValueEnum` (init.rs:22). Adding
   `McpLlm` auto-kebab-cases to `--as mcp-llm` and surfaces in `--help` via the
   doc-comment. AC1 feasible.
2. **TUI signature** — `config_tui::run(root: &Path, existing: Option<ExploreConfig>)`
   (config_tui/mod.rs:35). Plan's `config_tui::run(root, None)` matches. Both `init`
   and `config_tui` are binary-crate modules (`mod` in main.rs), so `crate::config_tui::run`
   is reachable from init.rs. AC2(b) feasible.
3. **`write_harness` shape** (init.rs:110) — an early-return `if target == McpLlm`
   branch cleanly precedes the existing `writes_mcp()`/`writes_steering()` logic;
   existing variants are structurally untouched (AC4).
4. **`write_mcp_json`** (init.rs:126) — the read/merge/`mcpServers[KEY]`/write pattern
   is exactly reusable for `write_mcp_json_explore` with `args:["serve","--explore"]`,
   preserving other servers (AC3).
5. **`claude_section`** (init.rs:173) is a two-arm `if writes_mcp() {} else {}`. Since
   `writes_mcp()` matches only `Mcp|Both` (init.rs:42), `McpLlm` would otherwise fall
   into the skill `else` arm — so the plan's explicit `McpLlm` arm *before* that check
   is required and correctly specified (AC3).
6. **Idempotency** — `write_claude_md` (init.rs:148) replaces content between
   `CLAUDE_START`/`CLAUDE_END` sentinels or appends; reusing the same sentinels for
   `AGENTS.md` gives idempotency for free (AC3, AC6).
7. **Non-TTY** — `IsTerminal`/`is_terminal()` is already the T05 pattern
   (config_tui/mod.rs:37); the init fast-fail guard is a proven, low-risk addition (AC5).
8. **Testing** — 7 unit + 3 integration tests trace to all 7 ACs, including
   backward-compat (existing init assertions unmodified) and the no-"fastcontext"
   check (AC7).

## Advisory notes (non-blocking)

1. **Dry-run ordering:** `run()` early-returns on `provisioned.is_empty()`
   (init.rs:67-69), which is the dry-run short-circuit. The McpLlm dry-run harness
   print (Step 3.1) must be placed *before* that return, or it will never execute.
   The plan acknowledges this but the implementation must place the block precisely.
2. **Non-TTY message wording (AC5):** `config_tui::run` already bails on non-TTY with
   a `grove config requires an interactive terminal` message. The init fast-fail guard
   is an optimization ahead of provisioning; ensure its message is worded for
   `grove init` (not verbatim "grove config…") while still matching AC5's intent.
   If the guard is skipped and the code simply relies on `config_tui::run` bailing, the
   error will be config-worded — acceptable but slightly off for init.
3. **`run()` output block:** keep `McpLlm.writes_mcp()` false so the generic
   "tools across its loop" ready block does not fire; the plan's dedicated McpLlm
   ready block (Step 3.4) is the correct path. Confirm `is_skill()`/`writes_steering()`
   arms in `run()` don't double-print for McpLlm.
