# GROVE-S03-T01: `GroveConfig` core type + explore section demotion + `mode→steering` rename

**Sprint:** GROVE-S03
**Estimate:** M
**Pipeline:** default

---

## Objective

Introduce `.grove/config.json` as grove's declared project-config file, modelled
by a new `GroveConfig` type in `core/`. This is the single source of truth for a
project's integration `mode`; today's `ExploreConfig` is demoted to an optional
`explore` section. Fold in the `explore.mode → explore.steering` rename so the
two `mode` axes (integration mode vs steering level) never collide. This task is
type + serde + persistence only — no consumer rewiring (T03) and no legacy
migration (T02) yet.

## Acceptance Criteria

1. A `GroveConfig` type in `core/` (new module, e.g. `core/src/config.rs`,
   exported from `core/src/lib.rs`) serialises/deserialises
   `{ "version": 1, "mode": <mode>, "explore"?: <explore-section> }` to/from
   `.grove/config.json`, following `ExploreConfig`'s existing pattern:
   raw-wire-shape deserialize with field-named enum errors, `validate()`,
   `load()`, and **atomic** `save()` (temp-file + rename, creating `.grove/`).
2. A new integration-`Mode` enum covers `mcp | skill | both | mcp-llm |
   grammars`, serialised in the on-disk spellings (`mcp-llm` etc.), with a
   `LEGAL` slice and a `from_name` that fails fast naming legal values (mirrors
   `Provider`/`Steering`). Distinct from the steering enum.
3. The existing steering enum (`config.rs` `Mode`: standard/balanced/aggressive)
   is **renamed to `Steering`**, and its JSON key on the explore section changes
   `mode → steering`. `Steering::LEGAL` and its `from_name` are preserved under
   the new name. All in-crate references updated (`agent.rs`, `steering.rs`,
   `toolset.rs`, `mcp.rs`, `config_tui/`, etc.) so the workspace compiles.
4. The `explore` section round-trips the existing provider / base_url / model /
   allowed_tools / tap / trace_retain fields plus the renamed `steering` field;
   it is present in the serialised config only when relevant (`Option<…>`,
   `#[serde(skip_serializing_if = "Option::is_none")]` or equivalent).
5. `GroveConfig::config_path(root)` = `<root>/.grove/config.json`. An illegal
   `mode` string is a descriptive load error, not a silent default; a missing
   file yields an actionable steer (as `ExploreConfig::load` does today).
6. Unit tests cover: serde round-trip for each mode; explore section present
   only when set; bad-`mode` error names the field + legal values; `steering`
   key (not `mode`) is what the explore section reads/writes; save→load
   round-trips atomically with no leftover temp file.
7. `cargo build` is warning-clean and `cargo clippy -- -D warnings` passes;
   `cargo test` green. Files end with a newline.

## Context

Implements items 1 & 2 (type + rename half) of `SPRINT_REQUIREMENTS.md` and the
Decision in ADR 0002 §Decision. Reference model:
`core/src/explore/config.rs` (`ExploreConfig` — copy its raw-shape / fail-fast /
atomic-save idiom). The steering enum being renamed is that file's `Mode`
(lines ~53–79). Intake decision **D2**: the rename ships in this sprint. Legacy
migration is deliberately **T02**; this task leaves `ExploreConfig::load` intact
and does not yet read `config.json` from any consumer.

## Artifacts Involved

- `core/src/config.rs` (new) — `GroveConfig`, integration `Mode`.
- `core/src/explore/config.rs` — `Mode`→`Steering` rename, `mode`→`steering`
  JSON key; likely re-home `ExploreConfig` as the `explore` section type (kept
  in place and re-used, not duplicated).
- `core/src/lib.rs` — module export.
- Call sites of the renamed steering enum across `core/` and `cli/`.

## Operational Impact

- **Version bump:** not required (unreleased/experimental surface; no new
  user-facing behaviour yet — consumers rewire in T03).
- **Regeneration:** none.
- **Security scan:** not required.
