# Validation Report — GROVE-S03-T01 (standalone review)

**Task:** GroveConfig core type + explore section demotion + mode→steering rename  
**Validator:** 🍵 grove QA Engineer — I validate against what was promised. The code compiling is not enough.

---

**Verdict:** Approved

---

## Acceptance Criteria Evaluation

### AC1 — GroveConfig type in core/, atomic persistence ✅ PASS

`core/src/config.rs` defines `GroveConfig` and is exported from `core/src/lib.rs` (`pub use config::{GroveConfig, Mode}`). The wire shape is `{version: 1, mode: <mode>, explore?: <explore>}`. Deserialization follows the raw-wire-shape pattern via `RawGroveConfig` with `String`-keyed enum fields, so field-named errors are possible. `validate()`, `load()`, and `save()` all present. `save()` creates `.grove/` via `create_dir_all`, writes to `config.json.tmp.<pid>`, then renames atomically.

Evidence: T6 (`save_load_round_trip_atomic`) asserts no leftover `.tmp.` file after a successful save and that `config.json` exists. Test passes.

### AC2 — Integration Mode enum with 5 variants ✅ PASS

`Mode` enum has `Mcp | Skill | Both | McpLlm | Grammars`. On-disk spellings via `#[serde(rename_all = "kebab-case")]`: `mcp`, `skill`, `both`, `mcp-llm`, `grammars`. `LEGAL` slice = `["mcp", "skill", "both", "mcp-llm", "grammars"]`. `from_name` fails fast with a message naming the offending value and listing all legal values. Plan-review advisory (`McpLlm` spelling) was heeded — variant is `McpLlm`, on-disk is `mcp-llm`. Distinct from `Steering`.

Evidence: T1 (`serde_round_trip_each_mode`) round-trips all 5 modes. T4 (`bad_mode_error_names_field_and_legal_values`) asserts error message includes `"mode"` field name and all 5 legal spellings. Both pass.

### AC3 — Steering rename (Mode→Steering, mode→steering key) ✅ PASS

`core/src/explore/config.rs`: enum renamed to `Steering`, `RawExploreConfig.steering: String`, `ExploreConfig.steering: Steering`. `steering.rs`, `agent.rs` updated. `explore/mod.rs` re-exports `Steering` (not `Mode`). CLI `config_tui/model.rs` imports `Steering`, uses `cfg.steering`. `config_tui/update.rs` uses `Steering::Aggressive`. `cli/src/mcp.rs`: `enum_str(&cfg.steering)`. `cli/tests/cli.rs` fixture JSONs all have `"steering"` (grep confirms 6 occurrences; no `"mode"` fixture keys remain).

Grep confirms no stray steering-Mode refs in explore or CLI code paths.

**Minor advisory (non-blocking):** `cli/src/config_tui/model.rs:124` doc comment reads `/// Index into \`Mode::LEGAL\`.` — should be `Steering::LEGAL`. Flagged by both plan-review and code-review as cosmetic; does not compile to a rustdoc link so no warning is emitted. No AC requires doc prose accuracy on internal type names.

### AC4 — explore section round-trips all fields; absent when None ✅ PASS

`GroveConfig.explore: Option<ExploreConfig>` is decorated `#[serde(skip_serializing_if = "Option::is_none")]`. `ExploreConfig` carries `provider / base_url / model / steering / allowed_tools / tap / trace_retain` — same as before the rename.

Evidence: T2 (`explore_section_absent_when_none`) asserts no `"explore"` key in the serialized JSON. T3 (`explore_section_present_when_some`) round-trips a full `ExploreConfig` embedded in `GroveConfig`. Both pass.

### AC5 — config_path, descriptive errors ✅ PASS

`GroveConfig::config_path(root)` returns `root.join(".grove").join("config.json")`. Illegal mode → T4 validates error message names field and lists legal values. Missing file → T7 (`missing_file_actionable_error`) asserts message contains `"grove init"` or `"grove config"`.

### AC6 — Unit test coverage for 6 categories ✅ PASS

All 6 required categories covered:

| Category | Test | Result |
|---|---|---|
| Serde round-trip for each mode | `serde_round_trip_each_mode` (T1) | ✅ |
| explore absent when None | `explore_section_absent_when_none` (T2) | ✅ |
| explore present when Some | `explore_section_present_when_some` (T3) | ✅ |
| bad-mode error names field + legal values | `bad_mode_error_names_field_and_legal_values` (T4) | ✅ |
| steering key reads/writes (not mode) | `steering_key_in_explore_section` (T5) | ✅ |
| save→load atomic, no leftover temp | `save_load_round_trip_atomic` (T6) | ✅ |

Bonus: `missing_file_actionable_error` (T7), `bad_version_rejected` (T8), and `legacy_mode_key_rejected` (in explore/config.rs) cover further edge cases. The legacy test explicitly asserts that a JSON with only `"mode"` (no `"steering"`) fails deserialization — correct T01 boundary per plan-review advisory.

### AC7 — Build clean, clippy -D warnings, cargo test green, files newline-terminated ✅ PASS

```
cargo test --release --locked:  151 + 77 + 28 + 1 = 257 passed, 0 failed
cargo clippy --all-targets -- -D warnings: Finished (no warnings)
Files: all 7 touched files end with 0x0a (newline)
```

---

## Edge Cases Probed

- **Bad mode string** → error names the field and lists all 5 legal values (T4).  
- **version != 1** → rejected by both `TryFrom<RawGroveConfig>` and `validate()` (T8, `bad_version_rejected`).  
- **Missing config file** → actionable error message steer (T7).  
- **Legacy `"mode"` key in explore section** → `RawExploreConfig.steering` is required with no default → deserialization fails with a missing-field error (T9 in explore/config.rs tests).  
- **Atomic save** → no leftover temp file after successful write (T6).  
- **explore: None absent from JSON** → `skip_serializing_if` confirmed (T2).  
- **All existing 257 tests pass** → no regression introduced.

---

## Summary

All 7 acceptance criteria are met and independently verified by running the toolchain. The implementation matches the approved plan and the plan-review advisories were heeded (McpLlm spelling, RawGroveConfig shape, version-1 gate, legacy-mode test). The sole cosmetic gap — stale `Mode::LEGAL` doc comment at `model.rs:124` — was acknowledged non-blocking by both earlier reviewers and generates no toolchain warning.
