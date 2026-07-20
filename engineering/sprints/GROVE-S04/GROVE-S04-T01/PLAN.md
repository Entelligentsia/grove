# PLAN — GROVE-S04-T01: Stage 1 — move `core/src/explore/` into a new `explore/` workspace crate

🗻 *grove Architect*

**Task:** GROVE-S04-T01  
**Sprint:** GROVE-S04  
**Estimate:** L

---

## Objective

Extract the entire `core/src/explore/` subsystem (11 modules + embedded prompt, ~3,900 lines) into a new workspace crate `explore/` (crate name `grove-explore-core`, not published). This is ADR 0004 Stage 1 — a mechanical boundary move. `grove-cst` returns to the pure structural library its crates.io description claims. The explore section of `GroveConfig` becomes opaque to core (`Option<serde_json::Value>`), no circular workspace dependencies are introduced, and all existing tests must pass.

---

## Approach

The boundary is already clean: `core/src/explore/mod.rs` exports only `run_explore[_reporting]`, config, and health types, with no outward tendrils into core's structural engine beyond `toolset.rs`'s use of `crate::ops`. The move is therefore mechanical.

**Dependency topology after T01:**

```
grove-explore-core ──dep──► grove-cst (grove_core::ops, grove_core::config, …)
grove-cst-cli      ──dep──► grove-cst + grove-explore-core
grove-cst          (no dep on grove-explore-core — avoids circular dep)
```

The critical design choice: `GroveConfig.explore` changes from `Option<ExploreConfig>` to `Option<serde_json::Value>`. This makes the explore section **opaque to core** and eliminates the need for `grove-cst` to depend on `grove-explore-core`. The CLI layer deserializes the `Value` into `ExploreConfig` (from the new crate) when it needs typed access.

`doctor.rs` remains in `grove-cst` but is adapted: the HTTP health probe calls (`health_probe`, `HealthError`) are removed (they require `ureq`-over-LLM from the new crate, which would re-introduce the circular dep). Config-based explore checks (validate presence and non-empty fields from the raw JSON Value) are retained. The `provider_reachable` / `model_served` checks move in scope to `grove-explore-core` for a later stage.

---

## Files to Modify / Create

### New files — `explore/` workspace crate

| File | Change | Rationale |
|---|---|---|
| `explore/Cargo.toml` | Create | New workspace crate manifest; lib crate named `grove_explore_core`; deps: `grove-cst`, `serde`, `serde_json`, `ureq`, `anyhow`, `dirs` |
| `explore/src/lib.rs` | Create | Crate root: `pub mod` declarations + public re-exports mirroring the current `core/src/explore/mod.rs` |
| `explore/src/mod.rs` | (absorbed into lib.rs) | The current `mod.rs` re-export block becomes `lib.rs` |
| `explore/src/agent.rs` | Move (byte-identical) | No external crate refs to change; `use super::*` paths unchanged |
| `explore/src/client.rs` | Move (byte-identical) | Only uses std, ureq, serde; all `super::` refs stay |
| `explore/src/config.rs` | Move (byte-identical) | Pure serde types; no external crate imports |
| `explore/src/discovery.rs` | Move (byte-identical) | Uses `super::health`; unchanged |
| `explore/src/grounding.rs` | Move (byte-identical) | No external crate refs |
| `explore/src/health.rs` | Move (byte-identical) | Uses ureq; all `super::` refs stay |
| `explore/src/steering.rs` | Move (byte-identical) | `include_str!` path must be adjusted to new location |
| `explore/src/toolset.rs` | Move (mechanical import change) | `use crate::ops` → `use grove_core::ops`; `use crate::explore::*` within module → `use super::*` (already the case) |
| `explore/src/trace.rs` | Move (byte-identical) | No external crate imports beyond serde/std |
| `explore/src/wire.rs` | Move (byte-identical) | Pure serde model |
| `explore/src/prompts/explore_v2.system.md` | Move | Byte-identical; `steering.rs` `include_str!` path must point to new location |

### Removed from `grove-cst`

| File | Change | Rationale |
|---|---|---|
| `core/src/explore/` (entire dir) | Remove | Subsystem moves to `explore/` crate |

### Modified — `grove-cst`

| File | Change | Rationale |
|---|---|---|
| `core/src/lib.rs` | Remove `pub mod explore;` and its re-exports (`ExploreConfig`, `Provider`, `Steering`); no new dep added | Explore module is gone; types were never needed here for core's own ops |
| `core/src/config.rs` | `GroveConfig.explore`: `Option<ExploreConfig>` → `Option<serde_json::Value>`; remove `use crate::explore::{ExploreConfig, Provider, Steering}`; rewrite `migrate_from_legacy_explore` to use pure JSON manipulation (rename `"mode"` → `"steering"` key in the raw Value); remove `LegacyExploreRaw` / typed parse — JSON key rename is sufficient; derive `PartialEq` only (drop `Eq`, since `serde_json::Value` is not `Eq`); update config round-trip tests to compare via `serde_json::Value` assertions | Opaque explore section; no circular dep |
| `core/src/doctor.rs` | Remove `use crate::explore::{health_probe, ExploreConfig, HealthError}`; remove `use crate::explore::toolset`; change `explore_checks(cfg: Option<&ExploreConfig>)` → `explore_checks(cfg: Option<&serde_json::Value>)`; inline known tool constants (`"Read"`, `"Glob"`, `"Grep"` literals); replace `c.validate()` with JSON field presence checks (base_url/model non-empty strings); **remove** `provider_reachable` and `model_served` health-probe checks; remove `provider_unreachable_is_fail` test (tests HTTP probe which is now outside scope of `grove doctor`); adapt `explore_config_absent_is_fail` test to work with `Option<serde_json::Value>` | Eliminates the only in-core dep on explore ureq/HTTP types; config-based doctor checks are retained |
| `core/Cargo.toml` | No dep changes required: `ureq` stays (used by `core/src/fetch.rs` for grammar downloads); all core deps are also needed by non-explore modules | ureq is not explore-only |

### Modified — workspace root

| File | Change | Rationale |
|---|---|---|
| `Cargo.toml` | Add `"explore"` to `workspace.members` | New workspace crate |

### Modified — `cli/`

| File | Change | Rationale |
|---|---|---|
| `cli/Cargo.toml` | Add `grove-explore-core = { path = "../explore", version = "=0.4.1" }` | CLI needs to access behavior types |
| `cli/src/mcp.rs` | Change `use grove_core::explore::{health_probe, ...}` → `use grove_explore_core::{health_probe, ...}`; deserialize `grove_cfg.explore` (`Option<serde_json::Value>`) into `ExploreConfig` via `serde_json::from_value` before passing to `health_probe` | Import path + Value deserialization |
| `cli/src/config_tui/model.rs` | Change `use grove_core::explore::{DiscoveredEngine, ENGINE_CANDIDATES}` → `use grove_explore_core::{DiscoveredEngine, ENGINE_CANDIDATES}` | Import path only |
| `cli/src/trace_tui/model.rs` | Change `use grove_core::explore::trace::{...}` → `use grove_explore_core::trace::{...}` | Import path only |
| `cli/src/init.rs` | Remove `use grove_core::ExploreConfig`; the explore fallback chain that calls `ExploreConfig::load(root)` must convert the result to `serde_json::Value` (`serde_json::to_value(ec)?`) before assigning to `explore: Option<serde_json::Value>` in the new `GroveConfig` | Type alignment: Value vs ExploreConfig |
| `cli/src/tap.rs` | Change `use grove_core::ExploreConfig` → `use grove_explore_core::ExploreConfig`; no functional change | Import path only; ExploreConfig::load/save still read/write `.grove/explore.json` (unchanged in the new crate) |

---

## Design Notes

### Why `Option<serde_json::Value>` for `GroveConfig.explore`?

The alternative — keeping `ExploreConfig` typed in `grove-cst` (e.g., in a new `core/src/explore_config.rs`) — leaves the LLM-config types (`Provider`, `Steering`) in the published library, which is the issue AC2 targets. Using `serde_json::Value` makes the explore section fully opaque: core serialises and deserialises it as a raw JSON object, with no knowledge of its internal structure. The CLI (which depends on both crates) is the only layer that needs to know the typed shape.

### `ExploreConfig::load` / `ExploreConfig::save` in the new crate

These methods **keep their current behaviour**: they read/write `.grove/explore.json` directly. The `GroveConfig.explore` section in `config.json` is the primary config surface for `grove serve`; `explore.json` continues to serve `grove tap` and `grove config` (the TUI writes via `ExploreConfig::save`). The two files can temporarily diverge (pre-existing post-migration situation; Stage 2 unifies the config surfaces).

### `steering.rs` — `include_str!` path

`steering.rs` contains `include_str!("prompts/explore_v2.system.md")`. This path is relative to the source file's location at compile time. After the move to `explore/src/steering.rs`, the prompt is at `explore/src/prompts/explore_v2.system.md` — the relative path `"prompts/explore_v2.system.md"` remains valid. No change needed.

### `doctor.rs` health probe removal

`grove doctor` currently probes the LLM provider (`provider_reachable`, `model_served`). After T01, these checks are removed from `grove doctor` because `health_probe` lives in `grove-explore-core` which `grove-cst` cannot depend on (circular). The health probe still fires at `grove serve` startup via `determine_surface` in `cli/src/mcp.rs`. A Stage 2 or dedicated task can add `grove-explore doctor` surface for provider health diagnostics.

### `cargo tree -p grove-cst` postcondition

After T01, `cargo tree -p grove-cst` must show no `ureq` edge that routes through explore-specific code (the ureq edge from `fetch.rs` remains). More precisely, no modules from `explore/` appear in `grove-cst`'s compiled output.

---

## Testing Strategy

1. **Compile check:** `cargo build --workspace` — must succeed with zero errors.
2. **Full test suite:** `cargo test --workspace` — all tests (including migrated explore unit tests in `explore/`) must pass.
3. **Clippy:** `cargo clippy --all-targets --workspace -- -D warnings` — zero warnings.
4. **Dependency audit:** `cargo tree -p grove-cst | grep explore` — must be empty (no explore crate in grove-cst's dep tree).
5. **Byte-identity verification:** diff of each moved behavior file (agent, client, health, discovery, grounding, trace, wire, steering) against the original, showing only changed `use` import paths and nothing else — recorded in `PROGRESS.md`.
6. **Config migration smoke test:** construct a temp dir with `.grove/explore.json` (legacy), call `GroveConfig::load`, confirm `config.json` is written and the explore `serde_json::Value` is present with a `"steering"` key (not `"mode"`).
7. **`cargo build --release --locked`** must produce a working `grove` binary.

---

## Acceptance Criteria

- [ ] New workspace member `explore/` compiles as a library crate; its `Cargo.toml` names it `grove-explore-core`.
- [ ] `core/src/explore/` directory is fully removed; `core/src/lib.rs` has no `pub mod explore;` line.
- [ ] `cargo tree -p grove-cst` shows no path to `grove-explore-core`.
- [ ] `GroveConfig.explore` is `Option<serde_json::Value>`; `core/src/config.rs` imports no types from the explore module/crate.
- [ ] `ExploreConfig`, `Provider`, `Steering` are defined in `explore/src/config.rs` (new crate); their serde and validation logic is byte-identical to the current `core/src/explore/config.rs` aside from the crate path.
- [ ] Behavior modules (`agent.rs`, `steering.rs`, `grounding.rs`, `toolset.rs`, `wire.rs`, `prompts/explore_v2.system.md`) are byte-identical aside from mechanical `use` import path changes (`crate::ops` → `grove_core::ops`).
- [ ] All existing explore unit tests move to the new crate and pass under `cargo test -p grove-explore-core`.
- [ ] `core/src/doctor.rs` compiles with no explore crate import; explore health-probe checks (`provider_reachable`, `model_served`) are removed; config-based checks remain.
- [ ] `cli/` changes are limited to import-path updates and `serde_json::from_value` deserialization where the CLI accesses `grove_cfg.explore`.
- [ ] `cargo test --workspace`, `cargo clippy --all-targets --workspace -- -D warnings`, and `cargo build --release --locked` all pass clean.
- [ ] Files end with a trailing newline.

---

## Operational Impact

- **Version bump:** Rides the next normal release train (D3 — not published to crates.io in this sprint).
- **Regeneration:** None — no user-facing behavior change.
- **Security scan:** Not required.
- **Migration:** None — zero user-visible behavior change; config file paths unchanged.
- **`cargo tree`:** The new `explore/` crate appears in `grove-cst-cli`'s dep tree but NOT in `grove-cst`'s. Any downstream consumer of the `grove-cst` library crate is unaffected.
