# PLAN — GROVE-S02-T01: Explore config model + persistence (`core::explore::config`)

🌱 *grove Engineer*

**Task:** GROVE-S02-T01
**Sprint:** GROVE-S02
**Estimate:** M

---

## Objective

Establish the foundational, typed configuration model for the mcp-llm inner
explorer subsystem: a serde-backed `ExploreConfig` (provider, endpoint, model,
steering mode, allowed-bash-tools allowlist) that loads from and saves to the
project-local file `.grove/explore.json`, with fail-fast validation and
sensible Ollama defaults. This is the shared vocabulary every other S02 task
(client, agent loop, serve mode, TUI, init) consumes, so it lands first. The
model stays deliberately minimal — mode-specific steering *content* (prompts)
is deferred to T03; this task only names the modes as an enum.

## Approach

Introduce a new `pub mod explore;` in `core/src/lib.rs` and implement the
configuration model in a dedicated `core/src/explore/config.rs` (with a thin
`core/src/explore/mod.rs` re-exporting the public types). The model comprises
three enums (`Provider`, `Mode`) plus the `ExploreConfig` struct, all deriving
`Serialize`/`Deserialize` (serde is already a core dependency). Persistence uses
the project-local `.grove/` directory — the same sibling convention as the
existing `.grove/grammars/` registry root — resolving the config path as
`root.join(".grove").join("explore.json")`.

Load/save/validate form the persistence surface:
- `ExploreConfig::load(root)` — read + deserialize + validate; a missing file
  yields a descriptive, actionable error steering the user to
  `grove init --as mcp-llm` or `grove config`.
- `ExploreConfig::save(root)` — validate, then serialize pretty JSON and write
  **atomically** (write to a sibling temp file in the same directory, then
  `rename` into place), creating `.grove/` if absent. This is a deliberate
  hardening over the existing registry writes, which use a plain
  `std::fs::write`; the atomic variant avoids a torn/partial config file if the
  process is interrupted mid-write, and AC #5 requires atomic-write behavior to
  be tested.
- `ExploreConfig::validate(&self)` — reject empty `base_url`/`model` and any
  structurally invalid state, naming the offending field and its legal values.

Enum deserialization is serde-driven: serializing enum variants to lowercase
strings (via `#[serde(rename_all = "lowercase")]`) means an out-of-range enum
value in the JSON produces a serde error that names the field and enumerates the
legal variants in the surfaced message. `anyhow` (already a core dep) carries
error context via `.with_context(...)`, consistent with `registry.rs` /
`fetch.rs`.

The crate must remain clap-free and contain no occurrence of the string
"fastcontext" — both are hard acceptance gates verified in step Testing.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `core/src/lib.rs` | Add `pub mod explore;` to the module list; optionally re-export `ExploreConfig`, `Provider`, `Mode` in the curated public surface | Declares the new subsystem and lets consumers name the config types via `grove_core::ExploreConfig` |
| `core/src/explore/mod.rs` | New — module root: `pub mod config;` and `pub use config::{ExploreConfig, Provider, Mode};` | Namespaces the mcp-llm engine (client/agent loop land here in later tasks) and re-exports the config surface |
| `core/src/explore/config.rs` | New — the `Provider`, `Mode` enums, `ExploreConfig` struct, `Default`/load/save/validate impls, and inline `#[cfg(test)] mod tests` | The task's core deliverable: the typed, persisted config model |

No existing files beyond `lib.rs` are touched; the change is purely additive.

## Data Model Changes

New types in `core::explore::config`:

- `enum Provider { Ollama, LlamaCpp }` — both speak the OpenAI-compatible
  transport; `Ollama` is the default. Serialized lowercase (`"ollama"`,
  `"llamacpp"`).
- `enum Mode { Standard, Balanced, Aggressive }` — the three steering levels
  (merit / plan-first / coerce per the sprint requirements). Serialized
  lowercase. Steering *content* is out of scope here (T03).
- `struct ExploreConfig { provider: Provider, base_url: String, model: String,
  mode: Mode, allowed_tools: Vec<String> }` — serde-backed, field names as the
  JSON keys.

`Default for ExploreConfig` produces the Ollama defaults:
`provider = Ollama`, `base_url = "http://localhost:11434/v1"`,
`model` = a sensible default identifier, `mode = Standard`,
`allowed_tools = ["grove", "rg", "grep", "find"]`.

New on-disk artifact: `.grove/explore.json` (project-local, sibling to
`.grove/grammars/`). No changes to `grove.lock`, `grove.json`, or any existing
schema. No migration of existing files.

## Testing Strategy

Inline `#[cfg(test)] mod tests` in `config.rs` covering AC #5:
- **serde round-trip:** `ExploreConfig::default()` → serialize → deserialize
  yields an equal value; and a fixed JSON fixture deserializes to the expected
  struct.
- **defaults:** `Default` gives the Ollama endpoint, `Standard` mode, and the
  exact `["grove", "rg", "grep", "find"]` allowlist.
- **validation errors:** missing-file load returns an error whose message
  mentions `grove init --as mcp-llm` / `grove config`; a JSON with a bad enum
  value produces an error naming the field and legal values; empty
  `base_url`/`model` is rejected.
- **atomic-write behavior:** `save` into a temp dir creates `.grove/explore.json`
  with valid content; a subsequent `load` round-trips; assert no leftover temp
  file remains beside it after a successful save.

Tests use unique temp dirs under `std::env::temp_dir()` keyed by process id
(same convention as `init.rs` / `registry.rs` tests) and clean up after
themselves.

Command gates (all must pass):
- `cargo build --release --locked --workspace` — warning-clean.
- `cargo clippy -- -D warnings` — clean.
- `cargo test --release --locked` — green (new tests + existing suite
  unaffected).
- `cargo tree -p grove-core` — shows **no** clap dependency.
- `grep -ri fastcontext core/` — returns nothing (naming constraint).

## Acceptance Criteria

- [ ] `core::explore` module exists (`pub mod explore;` in `core/src/lib.rs`)
      with `config.rs` exposing `ExploreConfig`, `Provider { Ollama, LlamaCpp }`,
      and `Mode { Standard, Balanced, Aggressive }`.
- [ ] `ExploreConfig::load(root)` / `save(root)` read/write `.grove/explore.json`
      with atomic (temp-file + rename) write.
- [ ] Validation fails fast with actionable messages: missing file steers to
      `grove init --as mcp-llm` / `grove config`; bad enum names field + legal
      values.
- [ ] `Default` yields the Ollama defaults; `allowed_tools` defaults to
      `["grove", "rg", "grep", "find"]`.
- [ ] Inline unit tests cover serde round-trip, defaults, validation errors,
      and atomic-write behavior.
- [ ] `cargo tree -p grove-core` shows no clap; the string "fastcontext"
      appears nowhere in the implementation.
- [ ] `cargo build --release --locked --workspace` warning-clean;
      `cargo clippy -- -D warnings` clean; `cargo test --release --locked` green.

## Operational Impact

- **Version bump required?** No — lands with the sprint's final release bump
  (per task prompt); this is an additive, not-yet-surfaced module.
- **Material change?** Yes — this is a schema/behavior addition to `grove-core`
  (a new persisted config file and public types). It is not docs-only.
- **Migration entry required?** No — no existing on-disk format changes; the new
  `.grove/explore.json` is created on demand by later tasks (init/TUI).
- **Distribution:** No user-facing behavior yet; no `/forge:update`-equivalent
  action required by users. No new crate dependencies (serde, serde_json,
  anyhow already present).
- **Backwards compatibility:** Fully additive — no existing surface, command,
  or file is touched, so no installed behavior can break.
