# ARCHITECT_APPROVAL — GROVE-S03-T01

**Verdict:** Approved

## Scope
Introduces `GroveConfig` (`core/src/config.rs`) with an integration `Mode` enum
(mcp|skill|both|mcp-llm|grammars) and an optional `ExploreConfig` sub-section, plus
the steering `Mode`→`Steering` rename and JSON key `mode`→`steering` across core and CLI.
Type/serde/persistence surface only.

## Architectural Assessment
- **Coherent with the stack.** Persistence follows the existing serde/serde_json
  convention (Stack §Core Dependencies). The `RawGroveConfig` + custom `Deserialize`
  pattern preserves field-named enum errors — consistent with how `ExploreConfig`
  already handles raw-wire validation, so no new deserialization idiom is introduced.
- **Config location is conventional.** `config_path` = `<root>/.grove/config.json`
  aligns with the project's `.grove/` cache/config layout (`dirs`-based OS resolution
  elsewhere; this is the repo-local project config).
- **Atomic save is correct posture.** Temp `config.json.tmp.<pid>` + `rename` with
  `create_dir_all` avoids torn writes — the right durability contract for a
  user-editable config file.
- **Rename is complete and contained.** grove callers/grep confirm no stray
  steering-`Mode` refs; the integration `Mode` and the unrelated TUI `Field::Mode`
  focus variant are correctly excluded. No cross-module leakage.

## Cross-Cutting Concerns
- **Clean T01/T02 boundary.** `legacy_mode_key_rejected` asserts the old explore
  `mode` key fails deserialization. This is the correct T01 behaviour — migration is
  T02's responsibility. The boundary is explicit and test-enforced, so no accidental
  backward-compat coupling was introduced here.
- No public released surface consumes `GroveConfig` yet; the integration `Mode` enum
  is additive and does not disturb existing explore/MCP call sites beyond the rename.

## Operational Impact
- **Version bump:** not required — unreleased/experimental config surface, no wire
  contract exposed to shipped clients.
- **Migration:** none in T01 (owned by T02).
- **Regeneration / security scan:** none required.
- **Deployment:** no topology or distribution changes.

## Follow-up Items
- **T02** owns legacy `mode`→`steering` migration for existing on-disk configs.
- **Cosmetic (non-blocking):** `cli/src/config_tui/model.rs:124` doc comment still
  reads `Mode::LEGAL`; update to `Steering::LEGAL` when next touching that file.
