# CODE_REVIEW — GROVE-S04-T01: Stage 1 — move `core/src/explore/` into a new `explore/` workspace crate (standalone review)

**Verdict:** Approved

## Scope reviewed

Extraction of the 11-module explore subsystem + prompt out of `grove-cst`
into a new `grove-explore-core` workspace crate, plus making
`GroveConfig.explore` an opaque `serde_json::Value` in core. Reviewed the
full working-tree diff (core + CLI), byte-identity of every moved module, and
independently re-ran the full verification suite.

## Independent verification (not taken from PROGRESS)

- **Byte-identity** — `diff` of every moved module against `git show HEAD:core/src/explore/*` confirms:
  - `discovery.rs`, `grounding.rs`, `health.rs`, `steering.rs`, `trace.rs`, `wire.rs`, `prompts/explore_v2.system.md` — **fully identical**.
  - `agent.rs`, `client.rs` — only test `use crate::explore::{wire,config}` → `crate::{wire,config}` (mechanical).
  - `config.rs` — only a doc-link `crate::explore::steering` → `crate::steering` (mechanical).
  - `toolset.rs` — sole non-path change `use crate::ops` → `use grove_core::ops`, exactly as planned. No other `crate::ops` / `crate::explore` refs remain in the crate.
- **`steering.rs` `include_str!`** — path `prompts/explore_v2.system.md` is source-relative and resolves in the new location (`explore/src/prompts/` present).
- **`lib.rs` re-export surface** — mirrors the old `mod.rs` re-export block exactly (only the module doc header changed); covers every symbol the CLI imports.
- **Architectural postcondition** — `cargo tree -p grove-cst | grep explore` → **empty** (verified); `grove-cst-cli` correctly gains `grove-explore-core v0.4.1`. The opaque-`Value` seam successfully breaks the circular dep.
- **Core decoupling** — `core/src/config.rs` and `core/src/lib.rs` import no explore types; `doctor.rs` has no explore-crate import and inlines the `"Read"/"Glob"/"Grep"` literals and field-presence checks as planned. Residual `provider_reachable`/`model_served`/`health_probe` mentions in core are comments only.
- **CLI changes** — limited to import repointing + `serde_json::{from_value,to_value}` conversions at the four boundaries (`mcp.rs` `determine_surface`/`call_explore_tool`, `init.rs` fallback chain, `config_tui/{mod,model,update}.rs`, `trace_tui/model.rs`, `tap.rs`). `mcp.rs` correctly handles a malformed explore Value by warning and falling back to `Surface::Standard` rather than panicking — good defensive handling.
- **Build/lint/test (re-run locally):**
  - `cargo clippy --all-targets --workspace -- -D warnings` → clean.
  - `cargo test --release --locked` → 135 + 112 + 33 + 54 + 1 doc = **335 passed, 0 failed**, matching PROGRESS.
  - New-crate + `explore/Cargo.toml` files all end with a trailing newline.

All 12 acceptance criteria are satisfied.

## Advisory notes (non-blocking)

1. **Stale intra-doc link.** `cli/src/config_tui/model.rs:107` still references
   `[`grove_core::explore::discover_engines`]` in a doc comment; the function now
   lives at `grove_explore_core::discover_engines`. Clippy/build do not catch broken
   intra-doc links, but `cargo doc` would warn. Repoint it in a follow-up (or when
   next editing that file). Recorded in the stack-checklist extraction item.
2. **Deferred legacy validation (accepted per plan, ADVISORY 3).**
   `migrate_from_legacy_explore` now does a pure `"mode"`→`"steering"` JSON key
   rename with no Provider/Steering validation; `GroveConfig::validate()` only checks
   `version == 1`. A malformed legacy `explore.json` migrates silently and only fails
   later at CLI `serde_json::from_value`. This matches the approved plan and is
   documented in the code and PROGRESS — noted, not a defect.
3. **Cosmetic PROGRESS labeling.** The test-evidence block labels the 135-test suite
   `grove-cst-cli` and the 112-test suite `grove-cst`; the actual runners are
   `grove_core` lib (135) and the `grove` cli bin (112). Numbers are correct; labels
   are swapped. Immaterial.

## Knowledge writeback

Added a **Workspace Structure** section to `engineering/architecture/stack-checklist.md`
capturing three reusable patterns this task established: the crate-layering
non-dependency (`grove-cst` ⊥ `grove-explore-core`), the opaque-`serde_json::Value`
seam for breaking circular workspace deps (and the resulting `Eq`→`PartialEq` drop),
and the byte-identical module-extraction discipline including stale intra-doc-link cleanup.
