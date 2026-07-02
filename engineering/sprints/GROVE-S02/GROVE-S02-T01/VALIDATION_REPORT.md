# VALIDATION_REPORT — GROVE-S02-T01: Explore config model + persistence (standalone review)

**Verdict:** Approved

Validated the `core::explore::config` deliverable against the seven task
acceptance criteria. All must-have criteria are met with evidence from the code
and an independent re-run of the gate suite.

## Per-criterion results

| # | Acceptance criterion | Result | Evidence |
|---|----------------------|--------|----------|
| 1 | `core::explore` module with `config.rs` exposing `ExploreConfig{provider,base_url,model,mode,allowed_tools}`, `Provider{Ollama,LlamaCpp}` (Ollama default), `Mode{Standard,Balanced,Aggressive}` | ✅ PASS | `core/src/lib.rs:43` `pub mod explore;`; `config.rs` defines all three types with the exact field set; `Provider::Ollama` is the `Default` provider |
| 2 | Load/save/validate against `.grove/explore.json` (sibling to `.grove/grammars/`), atomic write | ✅ PASS | `config_path` → `<root>/.grove/explore.json`; `load`/`save`/`validate` present; `save` writes to `explore.json.tmp.<pid>` then `fs::rename` — atomic; `save_then_load_round_trips_atomically` asserts no `.tmp.` leftover |
| 3 | Fail-fast actionable errors: missing file → `grove init --as mcp-llm`/`grove config`; bad enum names field + legal values | ✅ PASS | `load` bails with the steering message when file absent (`missing_file_steers_to_init`); `RawExploreConfig` + manual `Deserialize` yield `invalid \`provider\` value ...: expected one of ollama, llamacpp` (`bad_enum_names_field_and_legal_values`) |
| 4 | `Default` = Ollama defaults; `allowed_tools` = `["grove","rg","grep","find"]` | ✅ PASS | `Default` impl pins `http://localhost:11434/v1`, `qwen2.5-coder:7b`, `Standard`, and the exact tool list; `defaults_are_ollama` asserts each |
| 5 | Inline tests: serde round-trip, defaults, validation errors, atomic-write | ✅ PASS | 8 inline tests cover round-trip, fixed fixture, defaults, lowercase spellings, missing-file steer, bad-enum, empty-field validation, atomic save/load |
| 6 | `grove-core` clap-free; no "fastcontext" string | ✅ PASS | `cargo tree -p grove-cst` → no clap; `grep -rn fastcontext core/` → none |
| 7 | Build warning-clean; clippy `-D warnings` clean; test suite green | ✅ PASS | `cargo build --release --locked --workspace` → no warnings/errors; `cargo clippy --release --locked --workspace -- -D warnings` → clean; `cargo test --release --locked` → all pass (8/8 explore, full suite green) |

## Boundary / edge coverage assessment

- **Missing config file** — covered; steers to setup (AC #3).
- **Malformed enum values** — covered for `provider`; the same `from_name`
  mechanism guards `mode`. Field-named error verified.
- **Empty required fields** — `validate` rejects blank `base_url`/`model`
  (whitespace-only included via `.trim()`), covered by `empty_required_fields_rejected`.
- **Torn write on interrupt** — mitigated by temp+rename; test asserts no temp
  leak after success. Windows rename-over-existing caveat acknowledged in prior
  reviews; acceptable for the Unix-first target.

## Notes (non-blocking)

- AC #6 / PLAN reference `cargo tree -p grove-core`, but the crate package name
  is `grove-cst`. Verified against the real package name — intent (clap-free
  core) holds. Documentation nit only, already noted in code review.

No regressions observed; the change is purely additive (only `lib.rs` edited
among existing files). Task is validated.
