# PROGRESS — GROVE-S02-T01: Explore config model + persistence (`core::explore::config`)

## Summary of Changes

Implemented the typed, serde-backed `ExploreConfig` model persisted to the
project-local `.grove/explore.json` — the shared vocabulary for the S02 mcp-llm
inner explorer subsystem. Work is purely additive: a new `core::explore` module
namespaces the subsystem (client + agent loop land in later S02 tasks), and only
`core/src/lib.rs` was edited among existing files.

### What was built

1. **New module `core::explore`** (`core/src/explore/mod.rs`) — module root that
   declares `pub mod config;` and re-exports `ExploreConfig`, `Provider`, `Mode`.

2. **Config model** (`core/src/explore/config.rs`):
   - `enum Provider { Ollama, LlamaCpp }` — both speak the OpenAI-compatible
     transport; serialized lowercase (`"ollama"`, `"llamacpp"`), `Ollama` is the
     default.
   - `enum Mode { Standard, Balanced, Aggressive }` — the three steering levels;
     serialized lowercase. Steering *content* deferred to T03 (out of scope).
   - `struct ExploreConfig { provider, base_url, model, mode, allowed_tools }` —
     serde-backed; field names are the JSON keys.
   - `Default` yields the Ollama defaults: `provider = Ollama`,
     `base_url = "http://localhost:11434/v1"`, `model = "qwen2.5-coder:7b"`
     (pinned concrete value per review advisory), `mode = Standard`,
     `allowed_tools = ["grove", "rg", "grep", "find"]`.
   - `ExploreConfig::config_path(root)` → `<root>/.grove/explore.json` (sibling
     to the `.grove/grammars/` registry convention, `registry.rs:152`).
   - `ExploreConfig::load(root)` — read + deserialize + validate; a missing file
     yields an actionable error steering to `grove init --as mcp-llm` /
     `grove config`.
   - `ExploreConfig::save(root)` — validate, create `.grove/` if absent, then
     write **atomically** (temp file `explore.json.tmp.<pid>` in the same
     directory, then `rename` into place). This is a deliberate hardening over
     the existing registry writes, which use a plain `std::fs::write`.
   - `ExploreConfig::validate(&self)` — rejects empty `base_url`/`model`, naming
     the offending field and giving example legal values.

3. **lib.rs wiring** — added `pub mod explore;` to the module list and
   re-exported `ExploreConfig`, `Mode`, `Provider` in the curated public surface
   so consumers can name the types via `grove_core::ExploreConfig`.

### Design note — field-named enum errors (review advisory #1)

serde's stock unknown-variant error names neither the offending struct field nor
the struct. To satisfy AC #3 ("validation fails fast with actionable messages"),
`ExploreConfig` deserializes via a private `RawExploreConfig` whose `provider` /
`mode` are `String`s; a manual `Deserialize` impl converts through
`Provider::from_name` / `Mode::from_name`, which produce errors that name the
field (`` `provider` ``) and enumerate legal values (`ollama, llamacpp`). The
`bad_enum_names_field_and_legal_values` test asserts both.

The default `model` was pinned to a concrete value (`qwen2.5-coder:7b`, review
advisory #2) rather than left as a placeholder, and `defaults_are_ollama`
asserts it.

## Test Evidence

### `cargo test --release --locked` (full suite)

```
test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

### New explore tests (8, all green)

```
running 8 tests
test explore::config::tests::defaults_are_ollama ... ok
test explore::config::tests::bad_enum_names_field_and_legal_values ... ok
test explore::config::tests::fixed_fixture_deserializes ... ok
test explore::config::tests::empty_required_fields_rejected ... ok
test explore::config::tests::missing_file_steers_to_init ... ok
test explore::config::tests::provider_serializes_lowercase ... ok
test explore::config::tests::serde_round_trip_defaults ... ok
test explore::config::tests::save_then_load_round_trips_atomically ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 87 filtered out; finished in 0.00s
```

These cover AC #5 in full:
- **serde round-trip** — `serde_round_trip_defaults` + `fixed_fixture_deserializes`
- **defaults** — `defaults_are_ollama` (endpoint, mode, exact allowlist, pinned model)
- **validation errors** — `missing_file_steers_to_init`, `bad_enum_names_field_and_legal_values`, `empty_required_fields_rejected`
- **atomic-write behavior** — `save_then_load_round_trips_atomically` (asserts no leftover `.tmp.` file beside the config after a successful save)

### Command gates (all pass)

```
$ cargo build --release --locked --workspace
    Finished `release` profile [optimized] target(s) in 13.34s        # warning-clean

$ cargo clippy -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s)        # clean

$ cargo tree -p grove-cst | grep -i clap
PASS: no clap

$ grep -ri fastcontext core/
PASS: no fastcontext
```

## Acceptance Criteria — Status

- [x] `core::explore` module exists (`pub mod explore;` in `core/src/lib.rs`)
- [x] `ExploreConfig::load(root)` / `save(root)` read/write `.grove/explore.json`
- [x] Validation fails fast with actionable messages: missing file steers to `grove init --as mcp-llm` / `grove config`; bad enum names field + legal values
- [x] `Default` yields the Ollama defaults; `allowed_tools` defaults to `["grove", "rg", "grep", "find"]`
- [x] Inline unit tests cover serde round-trip, defaults, validation errors, atomic-write
- [x] `cargo tree -p grove-cst` shows no clap; the string "fastcontext" absent from `core/`
- [x] `cargo build --release --locked --workspace` warning-clean; clippy clean; tests green

## Files Changed

| File | Change |
| --- | --- |
| `core/src/lib.rs` | Added `pub mod explore;` and re-exported `ExploreConfig`, `Mode`, `Provider` |
| `core/src/explore/mod.rs` | New — module root, re-exports the config surface |
| `core/src/explore/config.rs` | New — `Provider`/`Mode` enums, `ExploreConfig` struct, `Default`/load/save/validate + inline `#[cfg(test)]` tests |

## Knowledge Writeback

None required — this is a purely additive new subsystem with no changes to
existing on-disk formats, schemas, or documented architecture. The atomic-write
hardening is local to the new `save` and does not alter the registry's
documented behavior.
