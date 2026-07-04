# CODE_REVIEW — GROVE-S03-T01 (standalone review)

**Verdict:** Approved

## Scope
`GroveConfig` core type + explore-section demotion + steering `mode→steering` rename.
Reviewed the uncommitted working-tree diff (10 modified files + new `core/src/config.rs`)
against the approved PLAN.md and the 7 acceptance criteria. All findings verified
independently by reading source and re-running the toolchain — not trusting PROGRESS.

## Acceptance-criteria compliance
| AC | Status | Evidence |
| --- | --- | --- |
| 1 — `GroveConfig` serde/load/save/validate/config_path, raw-wire deserialize, atomic save | ✅ | `config.rs`: `RawGroveConfig` + custom `Deserialize`, `validate()`, `load()`, atomic `save()` (temp `config.json.tmp.<pid>` + `rename`, `create_dir_all`) |
| 2 — integration `Mode` enum mcp\|skill\|both\|mcp-llm\|grammars, kebab spellings, `LEGAL` + fail-fast `from_name` | ✅ | `#[serde(rename_all="kebab-case")]`; `LEGAL` slice; `from_name` bails naming field + legal values. Variant correctly spelled `McpLlm` (plan-review advisory heeded) |
| 3 — steering `Mode`→`Steering` rename, JSON key `mode`→`steering`, all refs updated, workspace compiles | ✅ | `explore/config.rs`, `mod.rs`, `agent.rs`, `steering.rs`, `lib.rs`, `cli/config_tui/*`, `cli/mcp.rs`. grep confirms no stray steering-`Mode` refs remain (only integration `Mode` and the unrelated TUI `Field::Mode` focus variant) |
| 4 — explore section round-trips all fields incl. `steering`; `Option` + `skip_serializing_if` | ✅ | `explore: Option<ExploreConfig>` with `skip_serializing_if="Option::is_none"`; `explore_section_absent_when_none` + `_present_when_some` prove it |
| 5 — `config_path` = `<root>/.grove/config.json`; descriptive illegal-mode; actionable missing-file | ✅ | `config_path`, `bad_mode_error_names_field_and_legal_values`, `missing_file_actionable_error` |
| 6 — unit tests: 6 categories | ✅ | 8 tests in `config.rs` cover round-trip/absent/bad-mode/steering-key/atomic-save/missing-file + `bad_version_rejected`; explore side adds `legacy_mode_key_rejected` |
| 7 — clippy `-D warnings`, tests green, newline EOF | ✅ | Independently ran `cargo clippy --all-targets -- -D warnings` (clean) and `cargo test` (257 passed, 0 failed). All three touched files newline-terminated |

## Independent verification
- `cargo clippy --all-targets -- -D warnings` → clean.
- `cargo test` → 151 + 77 + 28 + 1 = 257 passed, 0 failed. PROGRESS test evidence is authentic.
- Confirmed all 8 `config::tests::*` execute and pass.

## Design notes (positive)
- Reuses `ExploreConfig`'s existing custom `Deserialize`, so embedding `Option<ExploreConfig>`
  preserves field-named inner errors — plan-review advisory followed (no `Option<serde_json::Value>`).
- `validate()` rejects `version != 1` (also enforced in `TryFrom`), matching AC5's fixed wire shape.
- `legacy_mode_key_rejected` explicitly asserts the old explore `"mode"` key fails — correct T01
  boundary (T02 owns migration).

## Advisory (non-blocking)
1. `cli/src/config_tui/model.rs:124` — doc comment `/// Index into `Mode::LEGAL`.` is now stale;
   the steering enum is `Steering`, so it should read `Steering::LEGAL`. Cosmetic only (plain
   backtick code, not an intra-doc link — no rustdoc warning, no compile impact). Flagged in the
   plan review too; safe to fold into a later touch of this file.
