# PROGRESS — GROVE-S03-T01
## GroveConfig core type + explore section demotion + mode→steering rename

## Summary

Implemented the full plan across 10 files (9 modifications + 1 new):

1. **`core/src/explore/config.rs`** — Renamed `Mode` enum → `Steering`, renamed field `mode` → `steering` on `ExploreConfig` and `RawExploreConfig`. Updated all internal references, tests, and fixtures. Added `legacy_mode_key_rejected` test asserting the old `"mode"` key is rejected.
2. **`core/src/explore/mod.rs`** — Swapped the `Mode` re-export to `Steering`.
3. **`core/src/explore/agent.rs`** — Updated import (`Mode` → `Steering`), `cfg.mode` → `cfg.steering`, `Mode::Balanced/Aggressive/Standard` → `Steering::...` throughout including test helper `cfg(mode: Mode)` → `cfg(steering: Steering)`.
4. **`core/src/explore/steering.rs`** — Updated import and all `Mode::` references; updated `system_prompt` parameter name (`mode` → `steering`).
5. **`core/src/lib.rs`** — Added `pub mod config;`; updated explore re-export from `Mode` → `Steering`; added `GroveConfig` + integration `Mode` re-exports.
6. **`core/src/config.rs`** *(new)* — Integration `Mode` enum (Mcp/Skill/Both/McpLlm/Grammars, kebab-case serde), `GroveConfig` struct with `version:u32`, `mode:Mode`, `explore:Option<ExploreConfig>` (skip_serializing_if none). `RawGroveConfig` with `mode:String` for field-named errors. `config_path`/`load`/`save` (atomic temp+rename, creates `.grove/`)/`validate` (version != 1 rejected). Full 7-test suite plus `bad_version_rejected`.
7. **`cli/src/config_tui/model.rs`** — `use grove_core::Mode` → `Steering`; `cfg.mode` → `cfg.steering`; `ExploreConfig { mode: ... }` → `steering:`.
8. **`cli/src/config_tui/update.rs`** — `grove_core::Mode::Aggressive` → `grove_core::Steering::Aggressive`; `mode:` → `steering:` in struct literal.
9. **`cli/src/mcp.rs`** — `enum_str(&cfg.mode)` → `enum_str(&cfg.steering)`.
10. **`cli/tests/cli.rs`** — Updated 6 explore.json fixture JSON objects from `"mode": "standard"` → `"steering": "standard"` (integration tests that write explore.json on disk).

## Test Evidence

```
running 151 tests
test config::tests::bad_mode_error_names_field_and_legal_values ... ok
test config::tests::bad_version_rejected ... ok
test config::tests::explore_section_absent_when_none ... ok
test config::tests::explore_section_present_when_some ... ok
test config::tests::missing_file_actionable_error ... ok
test config::tests::serde_round_trip_each_mode ... ok
test config::tests::save_load_round_trip_atomic ... ok
test config::tests::steering_key_in_explore_section ... ok
test explore::config::tests::bad_enum_names_field_and_legal_values ... ok
test explore::config::tests::defaults_are_ollama ... ok
test explore::config::tests::empty_required_fields_rejected ... ok
test explore::config::tests::fixed_fixture_deserializes ... ok
test explore::config::tests::legacy_mode_key_rejected ... ok
test explore::config::tests::missing_file_steers_to_init ... ok
test explore::config::tests::provider_serializes_lowercase ... ok
test explore::config::tests::save_then_load_round_trips_atomically ... ok
test explore::config::tests::serde_round_trip_defaults ... ok
test explore::config::tests::trace_retain_defaults_when_absent ... ok
test explore::agent::tests::balanced_recon_closes_grove_then_forces_submit_plan ... ok
test explore::agent::tests::progress_is_reported_each_turn_and_at_the_end ... ok
test explore::agent::tests::provider_down_maps_to_provider_down_error ... ok
test explore::agent::tests::standard_offers_the_four_execute_tools ... ok
test explore::agent::tests::standard_returns_first_text_only_turn_as_answer ... ok
test explore::agent::tests::turn_cap_forces_a_final_answer_not_a_sentinel ... ok
test explore::steering::tests::merit_and_coerce_select_distinct_grove_blocks ... ok
test explore::steering::tests::template_vars_are_rendered ... ok
... [125 more tests] ...
test result: ok. 151 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s

running 77 tests (cli unit)
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 28 tests (cli integration)
test tap_enables_tracing_in_config ... ok
test tap_no_enable_leaves_config_untouched ... ok
... [26 more tests] ...
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

Doc-tests grove_core
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

Total: 257 tests, 0 failures
```

### Clippy
```
cargo clippy -- -D warnings
Finished `dev` profile [optimized + debuginfo] target(s) in 1.72s
(no warnings)
```

### Release build
```
cargo build --release --locked
Finished `release` profile [optimized] target(s) in 12.56s
(no warnings)
```

## Files Changed

| File | Action |
|------|--------|
| `core/src/config.rs` | **CREATE** — new GroveConfig module |
| `core/src/explore/config.rs` | MODIFY — Mode→Steering rename, mode→steering field |
| `core/src/explore/mod.rs` | MODIFY — re-export Steering instead of Mode |
| `core/src/explore/agent.rs` | MODIFY — Mode→Steering, cfg.mode→cfg.steering |
| `core/src/explore/steering.rs` | MODIFY — Mode→Steering, system_prompt param |
| `core/src/lib.rs` | MODIFY — add config mod, update re-exports |
| `cli/src/config_tui/model.rs` | MODIFY — Mode→Steering, mode→steering field |
| `cli/src/config_tui/update.rs` | MODIFY — Mode::Aggressive→Steering::Aggressive |
| `cli/src/mcp.rs` | MODIFY — cfg.mode→cfg.steering |
| `cli/tests/cli.rs` | MODIFY — fixture JSONs "mode"→"steering" |

## Notes

- Integration `Mode` uses kebab-case serde so `McpLlm` → `"mcp-llm"` on wire (advisory from plan review heeded).
- `validate()` rejects `version != 1` in both `TryFrom` (deserialization path) and `validate()` method.
- Legacy explore `"mode"` key explicitly rejected — T02 owns migration.
- `explore` section absent from serialized form when `None` via `skip_serializing_if`.
