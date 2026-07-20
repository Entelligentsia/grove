# PLAN_REVIEW — GROVE-S04-T05 (standalone review)

**Verdict:** Approved

The revised plan resolves both blockers from the prior review and is grounded in
the real code. I independently verified every load-bearing claim.

## Independently verified

- **Blocker 1 fix (Step 8a) is real and correct.** `cli/src/init.rs:136` does call
  `crate::config_tui::run(root, None)?`, and `find_explore_binary()` exists at
  `init.rs:597`. Removing `config_tui` from the `cli` crate without Step 8a would
  break compilation (AC7). The exec-of-`grove-explore config` fix mirrors the
  existing T02 pattern.
- **Blocker 2 fix (Step 6) is real and correct.** `cli/src/config_tui/mod.rs:103`
  currently writes `mode: app.grove_mode`, which — combined with AC4's removal of
  the `grove_mode` field — would drop mode provenance. The revised save path
  re-reads `existing.mode` from `GroveConfig::load(root)`, mirroring the existing
  `harnesses` preservation (mod.rs:94-96). Round-trip test asserts `mode == Mcp`
  (preserved, not forced to `McpLlm`). Matches AC5.
- **Dependency set is complete.** The moved files (`config_tui/*`, `trace_tui/*`,
  `tap.rs`) import only `anyhow`, `crossterm`, `grove_core`, `grove_explore_core`,
  `ratatui`, `serde_json` (+`serde`) — every one present in the new
  `grove-explore/Cargo.toml` (Step 1). No hidden external dep.
- **Wholesale-move premise holds.** Zero `crate::{ops,engine,registry,init,serve,main}`
  references in the moved files — confirms the ADR's "no sibling-module coupling"
  finding, so the move will not leave dangling references.
- **String sweep targets match on-disk strings**: `tap.rs:24,26-27,33`,
  `trace_tui/mod.rs:4` (module doc), `config_tui/view.rs:90`. All old strings in
  Step 5 exist verbatim.
- **Dep removal targets real**: `cli/Cargo.toml` has `[[bin]] grove-explore`
  (lines 19-20) and `ratatui`/`crossterm` (lines 44-45); `GroveConfig` shape
  (`version`/`mode`/`explore`/`harnesses`) matches `core/src/config.rs:82`.

## Advisory notes (non-blocking — address during implementation)

1. **`explore_active` guard removal is broader than Step 4 spells out.**
   `update.rs` carries ~12 `if !app.explore_active { return None; }` guards
   (lines 25,31,40,45,52,58,64,118,125,134,146,153) and `view.rs` ~7 — not just
   the `Msg::Save` guard named in Step 4. This is compiler-enforced (the field is
   deleted, so all references must go), so completeness is guaranteed by AC7, but
   the engineer should expect to touch every guard, not only Save.

2. **First-run guard still keys on `explore.json` (Step 8a).** The `AFTER` snippet
   keeps `!root.join(".grove").join("explore.json").exists()` as the first-run
   trigger. Under ADR 0002 the config TUI writes `config.json`, not `explore.json`,
   so this guard effectively fires on every `grove init --as mcp-llm` first run —
   now exec'ing an *interactive* `grove-explore config`. This is pre-existing guard
   behavior (not a regression from this task), and the plan correctly notes the
   existing init tests seed `.grove/explore.json` to bypass it. Confirm no
   non-interactive init path (CI/scripted `grove init --as mcp-llm`) reaches the
   exec without that seed, else it will fail on the non-TTY guard.

3. **No-subcommand default to `serve` (Step 2).** `.mcp.json` invokes `grove-explore`
   with no args and must still start the server. clap does not auto-default to a
   subcommand — implement the `Option<Cmd>` → `Serve` fallback explicitly and keep
   an integration assertion that bare `grove-explore` (no subcommand) still serves,
   so the registration contract from T02 is not silently broken.

## Testing assessment

Coverage is adequate and targeted: AC5 round-trip test, both shim-spelling tests
(AC2 "either way"), non-TTY fail-fast, tap→`config.json` write, and the
`init_first_run_grove_explore_absent` error-path test. The `cargo tree` check
(AC6) directly proves the dep-tree split. All ACs map to concrete steps and tests.
