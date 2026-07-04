# PLAN_REVIEW — GROVE-S03-T05 (standalone review)

**Verdict:** Approved

The plan is feasible, complete, and correctly grounded in the actual codebase. I
verified every load-bearing assumption against source rather than the plan's
narrative.

## Independent verification performed

- **`grove_core::Mode`** (`core/src/config.rs:36`) — variants `Mcp, Skill, Both,
  McpLlm, Grammars` match the badge match arms exactly; derives `Copy`, so
  `match app.grove_mode` + moving `app.grove_mode` into a fresh `GroveConfig`
  both compile without a clone. ✓
- **`GroveConfig`** (`config.rs:75`) — fields `version: u32`, `mode: Mode`,
  `explore: Option<ExploreConfig>` match the plan's constructor/save snippets.
  `load(&Path)` / `save(&self, &Path)` signatures confirmed. ✓
- **Namespace safety** — the existing `App.mode: usize` (steering index) and
  `Field::Mode` (steering field) do NOT collide with the newly-imported
  `grove_core::Mode` type; the plan's `grove_mode` naming is the right call. ✓
- **`run` callers** — only two: `main.rs:302` and `init.rs:127`. Both are covered
  by the plan (main.rs switches to `GroveConfig::load`; init.rs keeps `None`,
  which stays valid under the new `Option<GroveConfig>` signature). ✓
- **`Msg` enum** — every variant the gating/tests reference (`ProviderUp/Down`,
  `UrlChar`, `ModelChar`, `ModeUp/Down`, `TapToggle`, `ToolsToggle`,
  `ToolsAddChar`, `TabNext/Prev`, `Save`, `Quit`) exists. ✓
- **Regression safety** — existing tests use `fresh() = App::default()`; the new
  `default()` routes through `from_grove_config(… McpLlm …)` →
  `explore_active = true`, preserving the fully-active path, so the "no existing
  test changes" claim holds. ✓
- **AC coverage** — all six ACs map to concrete changes; the "TUI never mutates
  mode" invariant is honoured (read-only `grove_mode`, no `Mode` selector added).

## Advisory notes (import hygiene — will fail `clippy -D warnings`, AC6)

These are self-correcting compile/lint issues, not design flaws, but flag them so
implementation stays warning-clean:

1. **mod.rs — do NOT remove the `ExploreConfig` import.** The Detailed Changes
   text says "Remove the now-unused `ExploreConfig` import," but `ExploreConfig`
   is still used at `mod.rs:105` (`fetch_models` builds `ExploreConfig { … }`).
   The plan's own mod.rs snippet correctly keeps it — follow the snippet, ignore
   the removal sentence. Removing it breaks compilation.

2. **mod.rs — drop `Mode` from the import list.** The snippet imports
   `use grove_core::{ExploreConfig, GroveConfig, Mode};`, but mod.rs never names
   `Mode` in its body (it reads `app.grove_mode` and builds
   `GroveConfig { mode: app.grove_mode, … }`). An unused `Mode` import fails
   `clippy -- -D warnings`. Import only `{ExploreConfig, GroveConfig}`.

3. **view.rs — add `use grove_core::Mode;`.** The badge `match app.grove_mode`
   arms name `Mode::Mcp` etc., but view.rs currently imports only
   `crate::config_tui::model::{App, Field}`. The plan mentions the `Mode` import
   under model.rs but not view.rs, which also needs it.

None of these change the plan's approach; they are surfaced at first `cargo build`
and are cheap to fix during implementation.
