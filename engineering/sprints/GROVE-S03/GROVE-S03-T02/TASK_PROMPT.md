# GROVE-S03-T02: Legacy `.grove/explore.json` migration on load (deprecated immediately)

**Sprint:** GROVE-S03
**Estimate:** M
**Pipeline:** default

---

## Objective

Ensure projects created before `.grove/config.json` existed keep working after
upgrade. When `config.json` is absent but the legacy `.grove/explore.json` is
present, `GroveConfig::load` migrates it forward once — synthesising
`mode: mcp-llm`, mapping the old `explore.mode` key to `explore.steering`, and
rewriting to `config.json`. Without this, every deployed `mcp-llm` project would
silently drop to Standard on upgrade (the exact failure ADR 0002 warns of). Per
intake decision **D4**, the legacy path is **deprecated immediately**: it warns
on use and is documented as slated for removal — not carried indefinitely.

## Acceptance Criteria

1. `GroveConfig::load(root)`: if `config.json` exists, load it (T01 behaviour);
   else if legacy `.grove/explore.json` exists, load + validate it, synthesise a
   `GroveConfig { version: 1, mode: mcp-llm, explore: Some(<migrated>) }`, and
   **rewrite forward** to `.grove/config.json` atomically.
2. The migration maps the legacy `explore.mode` steering key to
   `explore.steering` (the T01 rename), preserving the steering level.
3. On migration, a **deprecation warning** is emitted (stderr) stating the
   legacy `.grove/explore.json` is deprecated and that grove has migrated it to
   `.grove/config.json`; the warning is a one-time consequence of the rewrite
   (subsequent loads read `config.json` and do not warn).
4. When neither file exists, `load` returns the same actionable
   "run `grove init`" steer, unchanged.
5. Tests: (a) a legacy `explore.json` fixture (with old `mode` key) migrates to a
   `config.json` with `mode: mcp-llm` and `explore.steering` set; (b) after
   migration, `config.json` exists and a second `load` reads it without touching
   the legacy file; (c) a project with `config.json` already present ignores any
   stale `explore.json`; (d) the deprecation warning path is exercised.
6. `cargo build` warning-clean, `cargo clippy -- -D warnings` clean, `cargo test`
   green. Files end with a newline.

## Context

Implements item 2 (migration half) of `SPRINT_REQUIREMENTS.md` and ADR 0002
§Migration. Decision **D4**: retain the one-time auto-migrate + warning; do not
support the legacy file indefinitely. A concrete removal-target version is a
sprint nice-to-have, not required here. Depends on **T01** (the `GroveConfig`
type, integration `Mode`, and the `steering` field must exist).

## Artifacts Involved

- `core/src/config.rs` — `GroveConfig::load` migration branch; deprecation
  warning helper.
- `core/src/explore/config.rs` — reuse `ExploreConfig::load` to read the legacy
  file (its `config_path`/`load` stay for the legacy read).

## Operational Impact

- **Version bump:** required at release — this changes on-disk behaviour for
  existing `mcp-llm` projects (introduces `config.json`, deprecates
  `explore.json`).
- **Regeneration:** no explicit user action — migration is automatic on next
  `serve`/`load`; the deprecation warning informs the user.
- **Security scan:** not required.
