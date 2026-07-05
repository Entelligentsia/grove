# PLAN — GROVE-S03-T01
## GroveConfig core type + explore section demotion + mode→steering rename

---

## Objective

Introduce `GroveConfig` in `core/src/config.rs` as the new project-level
configuration type (`.grove/config.json`), demote `ExploreConfig` to an
optional `explore` sub-section within it, and rename the steering-level
`Mode` enum to `Steering` (with the JSON key changing from `"mode"` to
`"steering"` in the explore section). This is pure type + serde + persistence
work — no consumer rewiring and no legacy migration.

---

## Approach

The implementation follows the existing `ExploreConfig` idiom (raw wire-shape
struct + `TryFrom` + `Deserialize` impl, `validate()`, atomic `save()`,
actionable `load()` errors) and applies it to `GroveConfig`. The `Mode →
Steering` rename is a mechanical find-and-replace of the type name and field
names, touching all in-crate and in-CLI references to keep the workspace
compiling.

No new crate dependencies are introduced. The changes are scoped to the `core`
library crate and the `cli` binary crate; no contract-visible tool-spec or
workflow JSON changes occur.

---

## Files to Modify

| File | Action | What changes |
|------|--------|--------------|
| `core/src/config.rs` | **CREATE** | New module: integration `Mode`, `GroveConfig`, `RawGroveConfig`, all methods, unit tests |
| `core/src/lib.rs` | **MODIFY** | Add `pub mod config;`; re-export `GroveConfig` and integration `Mode`; change explore re-export from `Mode` → `Steering` |
| `core/src/explore/config.rs` | **MODIFY** | Rename `Mode` → `Steering`; field `mode` → `steering` on `ExploreConfig` and `RawExploreConfig`; update all internal refs and tests |
| `core/src/explore/mod.rs` | **MODIFY** | Re-export `Steering` instead of `Mode` |
| `core/src/explore/agent.rs` | **MODIFY** | `Mode` → `Steering` throughout; `cfg.mode` → `cfg.steering` |
| `core/src/explore/steering.rs` | **MODIFY** | `Mode` → `Steering` throughout; parameter name `mode` → `steering` |
| `cli/src/config_tui/model.rs` | **MODIFY** | `use grove_core::{..., Mode, ...}` → `Steering`; `cfg.mode` → `cfg.steering`; `ExploreConfig { mode: ... }` → `steering:` |
| `cli/src/config_tui/update.rs` | **MODIFY** | `grove_core::Mode::Aggressive` → `grove_core::Steering::Aggressive`; `mode:` → `steering:` in struct literal |
| `cli/src/mcp.rs` | **MODIFY** | `&enum_str(&cfg.mode)` → `&enum_str(&cfg.steering)` |

---

## Data Model Changes

### New: Integration `Mode` enum (`core/src/config.rs`)

```
pub enum Mode {
    Mcp,        // "mcp"
    Skill,      // "skill"
    Both,       // "both"
    MpcLlm,    // "mcp-llm"
    Grammars,   // "grammars"
}
```

On-disk spellings must match the hyphen-form used today (`"mcp-llm"` etc.).
Serialized with `serde(rename_all = "kebab-case")` or explicit rename
attributes. `Mode::LEGAL` slice + `from_name` pattern mirrors `Provider`.

### New: `GroveConfig` struct

```
pub struct GroveConfig {
    pub version: u32,                    // wire: "version": 1
    pub mode: Mode,                      // wire: "mode": "mcp"
    pub explore: Option<ExploreConfig>,  // wire: "explore"?: {...}
}
```

`explore` is `#[serde(skip_serializing_if = "Option::is_none")]` — omitted
from the serialized form when absent.

### New: `RawGroveConfig` struct

Holds `version: u32`, `mode: String`, `explore: Option<RawExploreConfig>` or
`Option<serde_json::Value>` to allow field-named deserialization errors for
the `mode` field.

### Modified: `ExploreConfig` field rename

- Field `pub mode: Mode` → `pub steering: Steering`
- `RawExploreConfig.mode: String` → `RawExploreConfig.steering: String`
- JSON on-disk key: `"mode"` → `"steering"` (breaking change on the explore
  section only; the legacy path is handled by T02, not this task)
- All derive/serialize attributes unchanged; no new fields added

### Modified: Steering enum rename

`pub enum Mode { Standard, Balanced, Aggressive }` → `pub enum Steering { … }`,
with all associated `impl`, `LEGAL`, and `from_name` preserved under the new
name. The `from_name` error message changes `"mode"` to `"steering"`.

---

## Implementation Order

1. **`core/src/explore/config.rs`** — rename `Mode` → `Steering`, rename
   field `mode` → `steering` in both the public struct and the raw wire struct.
   Update all internal uses (test fixtures, `try_from`, `default`, test
   assertions). This must compile cleanly before touching callers.

2. **`core/src/explore/mod.rs`** — swap the `Mode` re-export to `Steering`.

3. **`core/src/explore/agent.rs`** — update import and all `Mode::`/`cfg.mode`
   references to `Steering::`/`cfg.steering`.

4. **`core/src/explore/steering.rs`** — update import and all `Mode::`
   references (including the `system_prompt` signature) to `Steering::`.

5. **`core/src/lib.rs`** — add `pub mod config;`, update explore re-export.

6. **`core/src/config.rs`** — write the new module (integration `Mode`,
   `RawGroveConfig`, `GroveConfig`, methods, unit tests).

7. **`core/src/lib.rs`** (second pass) — add `GroveConfig` and integration
   `Mode` re-exports.

8. **CLI callers** — `cli/src/config_tui/model.rs`, `cli/src/config_tui/update.rs`,
   `cli/src/mcp.rs` — update `Mode` → `Steering`, `cfg.mode` → `cfg.steering`,
   field names in struct literals.

9. **`cargo build --release`** + **`cargo clippy -- -D warnings`** + **`cargo
   test`** — iterate until green.

---

## Testing Strategy

### Unit tests in `core/src/config.rs` (new)

- **`serde_round_trip_each_mode`** — for each integration `Mode` variant,
  serialize `GroveConfig { version: 1, mode, explore: None }` and deserialize
  back; assert equality.
- **`explore_section_absent_when_none`** — `ExploreConfig = None` must not
  emit the `"explore"` key (verify via `serde_json::to_value`).
- **`explore_section_present_when_some`** — optional section serializes and
  deserializes round-trip when set.
- **`bad_mode_error_names_field_and_legal_values`** — deserializing
  `{"version":1,"mode":"unknown"}` yields an error containing `"mode"` and
  all legal values.
- **`steering_key_in_explore_section`** — a `GroveConfig` JSON with
  `"explore": {"steering": "balanced", ...}` deserializes correctly; a
  `"mode"` key in the explore section must either be rejected or ignored
  (since that is the old spelling — T02 handles migration; T01 need only
  ensure the new key works).
- **`save_load_round_trip_atomic`** — `GroveConfig::save` + `::load`; assert
  no leftover temp file, and the loaded value equals the saved one.
- **`missing_file_actionable_error`** — `GroveConfig::load` on a missing
  file returns an error containing actionable guidance.

### Regression: existing `ExploreConfig` tests (in `core/src/explore/config.rs`)

All existing tests must pass after the rename. Key updated assertion:

- `provider_serializes_lowercase` must assert
  `serde_json::to_string(&Steering::Standard).unwrap() == "\"standard\""`.
- `fixed_fixture_deserializes` fixture JSON must use `"steering": "aggressive"`.
- Any fixture using `"mode"` in the explore object must be updated to
  `"steering"`.

### Compilation gate

`cargo build --release --locked` must pass warning-clean.
`cargo clippy -- -D warnings` must pass.
`cargo test` must be fully green.

---

## Acceptance Criteria Mapping

| AC | Covered by |
|----|------------|
| 1. `GroveConfig` serde/load/save/validate/`config_path` | `core/src/config.rs` new module |
| 2. Integration `Mode` enum with `LEGAL` + `from_name` | `core/src/config.rs` |
| 3. `Steering` rename, `mode→steering` JSON key, workspace compiles | all core + CLI changes |
| 4. `explore` section round-trips; absent when `None` | serde tests in `core/src/config.rs` |
| 5. `GroveConfig::config_path` = `<root>/.grove/config.json`; illegal mode descriptive | `core/src/config.rs` |
| 6. Unit tests — all six categories | `core/src/config.rs` tests + updated explore tests |
| 7. Clean build + clippy + tests green | CI gate |

---

## Operational Impact

- **Version bump:** not required — unreleased/experimental surface; no change
  to any stable public API.
- **Regeneration:** none — no tool specs altered.
- **Security scan:** not required.
- **Materiality:** the `.grove/explore.json` `"mode"` key rename is a
  breaking serialization change on an experimental, local-only file. Legacy
  migration (T02) handles deployed configs; T01 only needs to make the new
  key work. No stable binary contract is affected.

---

## Out of Scope (this task)

- Consumer rewiring: `serve`, `init`, `doctor`, `config` TUI mode-selection (T03, T05, T06).
- Legacy `.grove/explore.json` migration from old `"mode"` key to `"steering"` (T02).
- Reading `config.json` from any consumer code path (T03 onwards).
- Any changes to the seven structural tools or the `check` verb.
