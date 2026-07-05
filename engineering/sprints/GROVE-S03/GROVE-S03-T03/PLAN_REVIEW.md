# PLAN_REVIEW — GROVE-S03-T03 (standalone review)

**Verdict:** Approved

The plan is well-grounded in the actual source. I verified each load-bearing
claim against the code rather than the plan's narrative.

## Independent verification

1. **Shared resolver shape (AC-1).** `Mode` (`core/src/config.rs:36`) and
   `GroveConfig { version, mode, explore: Option<ExploreConfig> }` (:75) confirm
   `active_mode(root, ModeChoice) -> Mode` is buildable purely from existing
   config primitives. Precedence `ForceStandard → Mcp`, `ForceExplore → McpLlm`,
   `None → GroveConfig::load(root).mode` matches today's `determine_surface`
   semantics (`force_standard` short-circuits first). ✔
2. **`determine_surface` rewrite (AC-2).** Current body
   (`cli/src/mcp.rs:44`) reads `ExploreConfig::config_path(root).exists()` then
   `ExploreConfig::load`. The plan replaces both with `active_mode == McpLlm`
   and sources the explore section from `grove_cfg.explore` — valid because
   `GroveConfig.explore` is exactly `Option<ExploreConfig>`. ✔
3. **Bug-1 regression (AC-3).** `GroveConfig::load` (:210) checks
   `config.json` existence *before* the legacy `explore.json` branch. So
   `mode=mcp` + stale `explore.json` → `Mode::Mcp` → `Standard` immediately,
   never sniffing the stale file. The proposed unit + integration tests target
   this precisely. ✔
4. **Health-gated fallback preserved (AC-4).** Verified the existing test
   `explore_mode_unhealthy_provider_falls_back_to_standard_surface`
   (`cli/tests/cli.rs:424`) writes ONLY `explore.json`. Under the new path,
   `active_mode(None) → GroveConfig::load → migrate_from_legacy_explore`, and
   `migrate_from_legacy_explore` (:157) hard-sets `mode: Mode::McpLlm`.
   Therefore the McpLlm branch is still taken, `health_probe` still fires
   against the unreachable provider, and the test's `stderr.contains("falling
   back")` + 7-tool assertions still hold. The plan's "update comment only" for
   this test is correct — no behavioural test change needed. ✔
5. **Runtime override precedence (AC-5).** Mapping `(force_standard,
   force_explore) → ModeChoice` before calling `active_mode` preserves
   `--standard` winning over `--explore`. ✔
6. **Re-exports.** `core/src/lib.rs:68` currently
   `pub use config::{GroveConfig, Mode};` — extending to add `active_mode,
   ModeChoice` is consistent with the established pattern. ✔

## Advisory notes (non-blocking)

- **Side-effecting resolver.** `active_mode(None)` delegates to
  `GroveConfig::load`, which *migrates and writes `config.json`* (plus a
  deprecation warning) when only a legacy `explore.json` is present. So the
  "pure config resolver" described in Approach §1 is not side-effect free in the
  legacy path, and `doctor` calling `active_mode` will also trigger migration.
  This is consistent with existing `load()` semantics and harmless, but worth a
  one-line doc comment on `active_mode` so the next reader isn't surprised.
- **Double load is benign.** `determine_surface` loads `GroveConfig` a second
  time after `active_mode` already did. On the legacy path the first load
  migrates (writes config.json), so the second load reads the freshly written
  config.json — no double-migration, modes agree. The plan already flags the
  redundancy as acceptable; confirmed safe.
- **Non-explore declared modes.** `active_mode` can return `Skill`, `Both`, or
  `Grammars`; `determine_surface` maps all of these to `Standard` (only
  `McpLlm` yields explore). Correct for this task's scope — just make sure the
  `!= McpLlm → Standard` branch is written as an explicit catch-all so a future
  mode variant doesn't silently misroute.

## Testing assessment

Six unit tests (force variants, declared mcp/mcp-llm, bug-1 stale-file
regression, no-config fallback) plus the new `bug1_serve_mcp_mode_ignores_stale
_explore_json` integration test cover the AC matrix and the regression. Adequate
and meaningful. Ensure the bug-1 integration test asserts exactly 7 tools AND
absence of the explore delegating tool to lock the surface identity.
