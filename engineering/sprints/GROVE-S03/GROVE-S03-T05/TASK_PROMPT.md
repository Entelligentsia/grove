# GROVE-S03-T05: `config` TUI mode badge + inert explore-section rendering

**Sprint:** GROVE-S03
**Estimate:** S
**Pipeline:** default

---

## Objective

Make the `grove config` TUI display-consistent with the declared mode so it
never implies that explore settings drive a project that isn't in explore mode.
It gains read awareness of `mode` — but **no** mode selector. Switching mode
stays `grove init --as`, keeping `init` the single harness writer.

## Acceptance Criteria

1. The TUI reads the active `mode` from `GroveConfig` and shows it as a
   header/badge.
2. When `mode == mcp-llm`: the explore section is live and editable exactly as
   today (model-list dropdown, tap toggle, steering, etc.).
3. When `mode != mcp-llm`: the explore fields render inert/greyed with a
   one-line note that they are dormant until `grove init --as mcp-llm`. Inert
   fields are not editable and cannot be saved into an active explore config.
4. The TUI **never mutates `mode`** — there is no mode-selection control; it
   cannot become a second harness writer.
5. Existing `config` TUI tests pass; new coverage asserts the badge reflects the
   loaded mode and that the explore section is inert when `mode != mcp-llm`.
6. `cargo build` warning-clean, `cargo clippy -- -D warnings` clean, `cargo
   test` green. Files end with a newline.

## Context

Implements item 5 of `SPRINT_REQUIREMENTS.md` and ADR 0002 §Decision-3. Current
code: `cli/src/config_tui/` (`model.rs`, `update.rs`, `view.rs`, `mod.rs`) —
today it loads/saves `ExploreConfig`; it now loads a `GroveConfig` and renders
the explore section conditionally on `mode`. Depends on **T01** (`GroveConfig`,
`Mode`). Off the critical path — parallelisable with T02–T04.

## Artifacts Involved

- `cli/src/config_tui/model.rs` — hold the loaded `mode`; inert flag.
- `cli/src/config_tui/view.rs` — badge + greyed rendering + dormant note.
- `cli/src/config_tui/update.rs` — gate edits/save on `mode == mcp-llm`.

## Operational Impact

- **Version bump:** required at release (visible TUI change).
- **Regeneration:** none.
- **Security scan:** not required.
