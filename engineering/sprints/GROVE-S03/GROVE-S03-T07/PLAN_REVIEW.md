# PLAN_REVIEW — GROVE-S03-T07: `grove doctor` (standalone review)

**Verdict:** Approved

This is a re-review. The prior verdict (Revision Required) raised two blockers and one
advisory; all three are resolved in the revised plan, and every reused seam was
re-verified against the actual source tree.

## Blocker resolution (verified against source)

1. **Harness constant ownership** — Resolved. The plan introduces
   `core/src/harness.rs` as the single source of truth (`GROVE_START`, `GROVE_END`,
   `MCP_SERVER_KEY`, `expected_mcp_args`, `expected_claude_marker`,
   `agents_md_expected`), consumed by both `cli/src/init.rs` (writer) and
   `core::doctor` (reader). No duplication, no core→cli cycle. Verified the on-disk
   `cli/src/init.rs` currently owns `CLAUDE_START`/`CLAUDE_END` (l.20-21) and
   `MCP_SERVER_KEY` (l.79), and that the per-mode markers the plan encodes match the
   real steering blocks: `mcp__grove__outline` (mcp/both branch), `grove skill` (skill
   else-branch, l.536), `mcp__grove__explore` (McpLlm branch, l.457+). The
   `expected_mcp_args` mapping (`serve` / `serve --explore` / `None`) is consistent
   with the `Mode` enum (`Mcp, Skill, Both, McpLlm, Grammars`).

2. **Cross-crate private test helpers** — Resolved. Unit tests no longer reference
   the cli-private `reconcile_harness`/`seed_lock`; every fixture is built by hand in
   a `tempdir` via `std::fs::write` and fed to `diagnose`. The `.mcp.json` sub-check is
   pure JSON parsing (safe in core); CLAUDE.md/AGENTS.md checks operate on
   hand-written fixtures. The drift matrix (mcp+`--explore` args → Fail; mcp+explore
   marker → Fail; mcp-llm+absent AGENTS.md → Warn; grammars+GROVE_START → Warn) is
   coherent with the harness truth functions.

## Advisory resolution

3. **Warn-only exits zero (AC9 boundary)** — Incorporated as
   `warn_only_report_exits_zero`, asserting `Report::ok()` is true when only
   `Warn`/`Info` checks are present. `Report::ok()` correctly keys off `Status::Fail`
   only.

## Seam verification (all confirmed present, matching signatures)

- `active_mode(root, force: ModeChoice) -> Mode` (config.rs:291); `ModeChoice::{None,
  ForceExplore, ForceStandard}` matches the `--explore`/`--standard` precedence.
- `verify_lock(path) -> Result<Option<Vec<LockVerifyEntry>>>` (registry.rs:465) —
  supports the present/absent + Match/Mismatch/Missing mapping.
- `health_probe(&ExploreConfig) -> Result<(), HealthError>` with
  `HealthError::Unreachable{url,detail}` and `ModelMissing{model,url,available}`
  (client.rs) — matches the plan's provider_reachable / model_served enrichment.
- `list_models`, `ExploreConfig::validate`, `toolset::{READ,GLOB,GREP,GROVE}`,
  `registry::{search_path,cache_root,for_path,available}` all present.
- Global `--json` flag exists on the root CLI struct (main.rs:24), so AC6's
  dual-format exit-code contract is implementable.

## Advisory notes (non-blocking)

1. `harness_serve_surface` is emitted as `Info` only. AC3 phrases the boot surface as
   part of the headline harness-consistency check (fail-on-drift). This is acceptable
   because the surface is *derived* from mode + explore-section presence rather than an
   independent on-disk artifact that can drift, and the load-bearing drift (`.mcp.json`
   args) is already a hard `Fail` via `harness_mcp_json`. If a reviewer wants strict
   AC3 literalism, the implementer could promote a genuine surface mismatch to `Fail`;
   Info is a reasonable engineering call. Note there is no live `determine_surface`
   seam (only a historical comment at config.rs:610) — the implementer must compute
   the simulated surface inline from mode + explore presence.

2. Ensure the `diagnose(root, force: ModeChoice)` signature threads the CLI
   `explore`/`standard` bools into `ModeChoice` the same way `serve` does — the plan
   states "same precedence as serve" but does not show the bool→ModeChoice conversion;
   keep it identical to the serve dispatch to avoid a precedence divergence.

3. When both `--explore` and `--standard` are passed, decide and test the precedence
   (or reject) — a minor edge case worth an assertion.

These are advisory only and do not gate approval.
