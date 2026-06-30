# GROVE-S01-T06: crates.io publication readiness for grove-core + grove

**Sprint:** GROVE-S01
**Estimate:** M
**Pipeline:** default

---

## Objective

Make both workspace crates publishable to crates.io so library consumers can depend
on a released `grove-core` instead of a git path, and the `grove` CLI is installable
via `cargo install grove`. This is the **final gate** of the sprint — the code
restructure (T01–T05) is independent of and must not be blocked by publication.

## Acceptance Criteria

1. `core/Cargo.toml` and `cli/Cargo.toml` each carry the publish metadata crates.io
   requires: `description`, `license` (`MIT`, matching the current crate),
   `repository`, `homepage`, `readme`, `keywords`, `categories`.
2. `grove` depends on `grove-core` via **path + version**
   (`grove-core = { path = "../core", version = "=X.Y.Z" }`) so the published CLI
   resolves the published library at the matching version.
3. `cargo publish --dry-run -p grove-core` **and** `cargo publish --dry-run -p grove`
   both succeed (packaging, metadata, and dependency resolution validated; no actual
   publish in this task unless explicitly approved).
4. Crate-name availability/ownership on crates.io is confirmed for `grove` and
   `grove-core`. If either is taken by another owner, **stop and surface a fallback
   owned/namespaced name** (e.g. `entelligentsia-grove*`) for an explicit decision
   before any publish — do not silently rename.
5. Workspace build/test/clippy remain green; binary still `grove`; npm/Homebrew
   unaffected.

## Context

Depends on **T04** (curated public API + README that becomes the core crate's
`readme`) and **T05** (lockstep versioning via `bump-version.sh`, so the
path+version pin in criterion 2 stays consistent). Risk from requirements: the
`grove`/`grove-core` names may be taken — checking this is part of the task, and it
gates only publication. `cargo publish` ordering (core before cli) should be noted in
the task PROGRESS / RELEASING.md for the eventual real publish.

## Artifacts Involved

- Edited: `core/Cargo.toml`, `cli/Cargo.toml` (publish metadata + dependency pin).
- Possibly edited: `RELEASING.md` (document the core→cli publish order).
- Verify: `cargo publish --dry-run` for both packages.

## Operational Impact

- **Version bump:** publication uses the current synced version; no bump unless
  releasing.
- **Regeneration:** none for existing CLI/MCP/npm/Homebrew users.
- **Backward compat:** purely additive (a new crates.io presence); existing
  distribution channels unchanged.
