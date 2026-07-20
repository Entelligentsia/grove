# GROVE-S04-T05: Move config/trace TUIs + `tap` to `grove-explore`; forwarding shims + string sweep

**Sprint:** GROVE-S04
**Estimate:** M
**Pipeline:** default

---

## Objective

Move the two pure explore-product UIs — `cli/src/config_tui/` and
`cli/src/trace_tui/` + `cli/src/tap.rs` (~2,950 lines) — wholesale to the
`grove-explore` binary as `grove-explore config` and `grove-explore tap`,
leaving one-release forwarding shims at the old `grove` spellings, and sweep
the user-facing strings that still tell the single-server story.

## Acceptance Criteria

1. `grove-explore config` and `grove-explore tap` provide today's TUIs
   unchanged in function (engine discovery picker, model dropdown, Tap toggle;
   session→call→turn trace browser).
2. `grove config` and `grove tap` remain for **one release** as forwarding
   shims that invoke the new verbs (or print the new spelling if
   `grove-explore` is absent) — each shim prints the new spelling either way.
3. String sweep lands in the same change: `tap`'s hint reads "restart the
   grove-explore server" (not "restart `grove serve`"); its error path no
   longer references the ADR-0002-deprecated `.grove/explore.json`.
4. ADR 0002 §3's mode-gated inert rendering (S03-T05) is removed as moot: the
   explore config is always live. Any badge reflects whether the
   `grove-explore` server is registered in `.mcp.json`, not the mode
   (nice-to-have; removal of the mode badge/inert path is the must-have).
5. **Config ownership (interim):** `config_tui` keeps its exact
   read-modify-write of grove-owned `.grove/config.json` — preserving `mode`
   and `harnesses`, rewriting only the explore section — covered by a config
   round-trip test (the guest-writer skew risk).
6. ratatui/crossterm are dependencies only of `grove-explore` after this task
   (`cargo tree` check with T03); the `grove` binary has no TUI deps.
7. Workspace green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean build, files end with a newline.

## Context

Implements item 5 of `SPRINT_REQUIREMENTS.md`; ADR 0004 Stage 2 (TUI
surfaces). Depends on **T02** (the `grove-explore` binary hosts the verbs).
The ADR verified both TUIs import only explore types (`ExploreConfig`/
`Provider`/`Steering`, discovery, `/models` fetch; trace helpers + tap flag) —
no `ops`/`engine`/`registry` references — so this is a wholesale move, not a
refactor. `grove tap`'s `--no-enable` flag and TTY requirements carry over
unchanged. UX memory applies: the TUIs must keep their current
liveness/progress behavior.

## Artifacts Involved

- `cli/src/config_tui/`, `cli/src/trace_tui/`, `cli/src/tap.rs` — moved to the
  grove-explore bin's source tree
- `cli/src/main.rs` — forwarding shims for `config`/`tap`
- `cli/Cargo.toml` — drop ratatui/crossterm; grove-explore's manifest gains them
- Tests: shim behavior, config round-trip, moved TUI unit tests

## Operational Impact

- **Version bump:** rides the next release train; shim deprecation noted in
  CHANGELOG with the removal release named.
- **Regeneration:** none — old spellings keep working for one release.
- **Security scan:** not required.
