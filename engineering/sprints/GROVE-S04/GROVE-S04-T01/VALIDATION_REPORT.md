# VALIDATION REPORT — GROVE-S04-T01
## Stage 1 — move `core/src/explore/` into a new `explore/` workspace crate

*(standalone review)*

**Verdict:** Approved

---

## Acceptance Criteria Checklist

### AC1 — New workspace member `explore/` holds all 11 modules + prompt; public surface preserved ✅ PASS

Verified via `ls explore/src/`:

```
agent.rs client.rs config.rs discovery.rs grounding.rs health.rs
steering.rs toolset.rs trace.rs wire.rs prompts/explore_v2.system.md
```

`explore/src/lib.rs` re-exports exactly the old `mod.rs` boundary:
`run_explore`, `run_explore_reporting`, `ExploreAnswer`, `ExploreError`,
`NoopReporter`, `ProgressReporter`, `ChatClient`, `ClientError`,
`OpenAiCompatClient`, `discover_engines`, `DiscoveredEngine`,
`EngineCandidate`, `ENGINE_CANDIDATES`, `health_probe`, `list_models`,
`HealthError`, `ChatRequest`, `ChatResponse`, `Message`, `Role`, `Tool`,
`ToolCall`, `Usage`, `ExploreConfig`, `Provider`, `Steering`,
`SessionMeta`, `TraceWriter`.

Crate name confirmed: `grove-explore-core` / `grove_explore_core`.

---

### AC2 — `grove-cst` contains no explore modules; dep tree clean ✅ PASS

- `core/src/explore/` directory: **absent** (`ls: cannot access 'core/src/explore/': No such file or directory`)
- `core/src/lib.rs`: zero references to `pub mod explore`, `ExploreConfig`, `Provider`, or `Steering` (grep returns empty)
- `cargo tree -p grove-cst | grep explore`: **empty** — no path to `grove-explore-core` in the `grove-cst` dep tree
- `cargo tree -p grove-cst-cli | grep explore`: shows `├── grove-explore-core v0.4.1` (correct — only the CLI binary depends on it)

---

### AC3 — `GroveConfig.explore` is an opaque `serde_json::Value`; config load/save/migration unchanged ✅ PASS

`core/src/config.rs` line 93: `pub explore: Option<serde_json::Value>`

No imports of `ExploreConfig`, `Provider`, or `Steering` anywhere in `core/src/config.rs`. `migrate_from_legacy_explore` is a pure JSON key-rename (`"mode"` → `"steering"`) with no typed parse. `GroveConfig` derives `PartialEq` only (correct: `serde_json::Value` is not `Eq`). Config round-trip tests updated to Value assertions. Tests pass.

---

### AC4 — Behavior-bearing modules move byte-identical (aside from mechanical path adjustments) ✅ PASS

Verified via `git diff` of each file against HEAD (original `core/src/explore/` version):

| Module | Result |
|---|---|
| `grounding.rs` | **byte-identical** |
| `steering.rs` | **byte-identical** |
| `wire.rs` | **byte-identical** |
| `prompts/explore_v2.system.md` | **byte-identical** |
| `toolset.rs` | 1 line: `use crate::ops` → `use grove_core::ops` (sole non-path content change, as documented) |
| `agent.rs` | 2 test `use` lines: `crate::explore::wire` → `crate::wire`, `crate::explore::config` → `crate::config` (mechanical path adjustments) |
| `config.rs` | 1 intra-doc link: `crate::explore::steering` → `crate::steering` (mechanical path adjustment) |
| `client.rs`, `health.rs`, `discovery.rs`, `trace.rs` | byte-identical |

All changes are purely mechanical; no logic was modified.

---

### AC5 — CLI changes limited to import-path updates and `serde_json::from_value` deserialization ✅ PASS

Verified import repointing in all CLI files:

| File | Verified |
|---|---|
| `cli/src/mcp.rs` | `use grove_explore_core::{health_probe, run_explore_reporting, ExploreConfig, ExploreError, OpenAiCompatClient, NoopReporter, ...}` |
| `cli/src/init.rs` | `use grove_explore_core::ExploreConfig`; result converted via `serde_json::to_value(ec)` |
| `cli/src/tap.rs` | `use grove_explore_core::ExploreConfig` |
| `cli/src/config_tui/mod.rs` | `use grove_explore_core::ExploreConfig`; calls `grove_explore_core::discover_engines()`, `grove_explore_core::list_models()`; saves via `serde_json::to_value` |
| `cli/src/config_tui/model.rs` | `use grove_explore_core::{DiscoveredEngine, ENGINE_CANDIDATES, ExploreConfig, Provider, Steering}`; `from_grove_config` deserializes via `serde_json::from_value` |
| `cli/src/config_tui/update.rs` | `use grove_explore_core::ExploreConfig`; `Provider`/`Steering` from `grove_explore_core` |
| `cli/src/trace_tui/model.rs` | `use grove_explore_core::trace::{format_response, request_parts, traces_dir}` |

In `mcp.rs`, malformed Value → falls back to `Surface::Standard` with a warning (no panic) — error path tested indirectly via clippy + test coverage.

---

### AC6 — All explore unit tests move; full workspace green; files end with newline ✅ PASS

**`cargo test -p grove-explore-core --release --locked`**: 54 passed, 0 failed

**`cargo test --release --locked` (full workspace)**:
```
grove-cst (core):  135 passed, 0 failed
grove-cst-cli:     112 passed, 0 failed
grove-cst (lib):    33 passed, 0 failed
grove-explore-core: 54 passed, 0 failed
doc-tests (core):    1 passed, 0 failed
doc-tests (explore): 0 passed, 0 failed
                   ──────────────────────
Total:             335 passed, 0 failed
```

**`cargo clippy --all-targets --workspace -- -D warnings`**: no errors, no warnings (clean)

**`cargo build --release --locked`**: `Finished release profile`

**Trailing newlines**: all 12 files in `explore/src/` (including `prompts/explore_v2.system.md`) verified to end with `\n`.

---

## Advisory Notes (non-blocking)

**Stale intra-doc link** (`cli/src/config_tui/model.rs:107`): The doc comment reads `grove_core::explore::discover_engines` — should be `grove_explore_core::discover_engines`. Identified in code review; `cargo build`/`clippy` do not fail on it but `cargo doc` would emit a warning. Does not affect any acceptance criterion.

**Silent legacy migration** (`migrate_from_legacy_explore`): Provider/Steering validation is deferred to CLI deserialization as documented. A malformed legacy file migrates without error but fails cleanly at the CLI boundary. Noted in plan review and implementation; within scope of the plan's documented trade-off.

---

## Summary

All 6 acceptance criteria verified against the live codebase with direct evidence:
- Filesystem structure confirmed
- `cargo tree` postcondition confirmed (zero explore dep in `grove-cst`)
- Type seam (`Option<serde_json::Value>`) confirmed in source
- Byte-identity of behavior-bearing modules confirmed via diff
- CLI import repointing confirmed in all 7 affected files
- 335 tests pass, 0 fail; clippy clean; release build passes; all files end with newline
