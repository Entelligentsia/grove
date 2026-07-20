# PLAN REVIEW — GROVE-S04-T01 (standalone review)

**Verdict:** Approved

Stage 1 of ADR 0004: move `core/src/explore/` (11 modules + prompt) into a new
`explore/` workspace crate and make `GroveConfig.explore` opaque
(`Option<serde_json::Value>`) so `grove-cst` sheds all LLM/explore deps without a
circular workspace edge. The plan is architecturally sound and feasible. I
verified the load-bearing claims against the actual tree before approving; the
advisory notes below are enumeration completeness items the engineer should fold
in to avoid a wasted build/clippy iteration — none require a structural rewrite.

## Independently Verified (holds)

1. **Circular-dep avoidance is correct.** `grove-explore-core → grove-cst` with
   `GroveConfig.explore: Option<serde_json::Value>` keeps core ignorant of the
   explore shape. `core/src/explore/mod.rs` already exports only
   `run_explore[_reporting]`, config, health, and discovery types — the boundary
   is genuinely clean, and `toolset.rs`'s only outward tie is `use crate::ops`
   (in `grove_json`, a non-test fn at line 339) → `use grove_core::ops`.
2. **Inline tool constants are correct.** `toolset::READ/GLOB/GREP` =
   `"Read"/"Glob"/"Grep"` — the plan's inlined literals in `doctor.rs` match
   exactly; no behavior change.
3. **Version pin is correct.** Workspace is at `0.4.1`; the proposed
   `grove-explore-core = { path = "../explore", version = "=0.4.1" }` mirrors the
   existing `grove-cst` pin in `cli/Cargo.toml`.
4. **`include_str!` path survives.** `steering.rs` uses
   `include_str!("prompts/explore_v2.system.md")` — source-relative, so after the
   move to `explore/src/steering.rs` with `prompts/` alongside it, the path stays
   valid. No change needed (plan agrees).
5. **`validate()` loses nothing.** `GroveConfig::validate()` only checks
   `version == 1`; it never inspected the explore section, so making it opaque
   removes no validate()-time coverage.
6. **New crate's public surface is sufficient.** `mod.rs` already re-exports every
   symbol the CLI imports (`health_probe`, `run_explore_reporting`, `ExploreConfig`,
   `ExploreError`, `OpenAiCompatClient`, `NoopReporter`, `DiscoveredEngine`,
   `ENGINE_CANDIDATES`, `discover_engines`, `list_models`, `Provider`, `Steering`,
   `trace::*`), so mirroring it into `explore/src/lib.rs` covers all consumers.

## Advisory Notes (fold into implementation — will otherwise break the build)

1. **CLI file inventory is incomplete.** The plan's "Modified — cli/" table lists
   `mcp.rs`, `config_tui/model.rs`, `trace_tui/model.rs`, `init.rs`, `tap.rs`, but
   the following call sites also reference `grove_core::explore` /
   `grove_core::{ExploreConfig,Provider,Steering}` and must be repointed to
   `grove_explore_core` or they will fail to compile once the re-exports are
   dropped from `core/src/lib.rs`:
   - `cli/src/config_tui/mod.rs` — `use grove_core::{config::GroveConfig, ExploreConfig}` (L24), `grove_core::explore::discover_engines()` (L56), `grove_core::explore::list_models(&cfg)` (L128).
   - `cli/src/config_tui/update.rs` — `use grove_core::ExploreConfig` (L190; also test uses at L354, L450).
   - `cli/src/config_tui/model.rs` **line 4** — `use grove_core::{config::GroveConfig, ExploreConfig, Provider, Steering}`. The plan only calls out the L3 `DiscoveredEngine/ENGINE_CANDIDATES` import on this file; the L4 import of `ExploreConfig/Provider/Steering` must move too.
   The plan's AC "cli/ changes are limited to import-path updates" stays true —
   this is purely completing the list. The mandated `cargo build --workspace`
   gate will surface any omission, but enumerating them now saves an iteration.

2. **`mcp.rs` uses `NoopReporter` (L374) and `ExploreConfig` (L36) beyond the L15-16
   import block.** These resolve through the new crate's re-exports; just confirm
   the repointed `use` covers `run_explore_reporting`, `ExploreConfig`,
   `ExploreError`, `OpenAiCompatClient`, `health_probe`, **and** `NoopReporter`.

3. **Migration validation semantics shift.** Today `migrate_from_legacy_explore`
   calls `Provider::from_name`/`Steering::from_name` (which `bail!` on invalid
   legacy values, and map the deprecated `"aggressive"` → `Strict` alias) before
   persisting. A pure JSON key-rename (`mode`→`steering`) stores the raw values
   verbatim and defers validation to the CLI's `serde_json::from_value::<ExploreConfig>`
   (whose custom `Deserialize` at `explore/config.rs:153` still routes through
   `from_name`, so the `"aggressive"` alias is preserved — good). Net effect: a
   *malformed* legacy `explore.json` that previously failed loudly at migrate time
   will now migrate silently and only error when the CLI first deserializes the
   section. This is consistent with the opaque-core design but is a behavior delta
   against AC3's "migration behavior unchanged" — acceptable, but note it in
   PROGRESS.md and keep the existing `migrate_legacy_explore_writes_config_json`
   test green (it uses `mode:"balanced"`, which round-trips cleanly under the
   rename).

4. **Byte-identity backstop.** AC4 requires a per-file diff summary in PROGRESS.md
   proving the behavior modules moved byte-identical apart from mechanical `use`
   changes. `toolset.rs` is the only file with a genuine import edit
   (`crate::ops` → `grove_core::ops`); the diff summary should make that the sole
   non-path delta for that file, and empty (path-only) for the rest.

## Testing — adequate
The strategy (workspace build/test/clippy, `cargo tree -p grove-cst | grep explore`
empty, byte-identity diffs, legacy-migration smoke test, `--release --locked`
build) covers the acceptance criteria and the circular-dep postcondition. No gaps.
