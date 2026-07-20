# ARCHITECT_APPROVAL.md — GROVE-S04-T05

**Task:** Move config/trace TUIs + `tap` to `grove-explore`; forwarding shims + string sweep

**Verdict:** Approved

## Rationale

This task cleanly realizes an intended architectural boundary: the interactive
TUI surfaces (`config_tui`, `trace_tui`) and the `tap` toggle now live in a
dedicated `grove-explore` workspace crate, and the heavy TUI dependencies
(`ratatui`/`crossterm`) are isolated to that crate. I independently confirmed
the on-disk state:

- `grove-explore/src/` contains `config_tui`, `trace_tui`, `tap.rs`, `main.rs`.
- `cli/Cargo.toml` carries no `ratatui`/`crossterm` (AC6 dependency isolation).
- `Cargo.toml` workspace `members` includes `grove-explore`.

All 7 acceptance criteria were re-confirmed by an independent code review
(verdict: approved) and validation (verdict: approved — 340 tests passed, 0
failed; clippy `--all-targets --workspace -D warnings` clean). The prior AC7
cold-target CI blocker is resolved and was reproduced by rm-ing the release
binary and observing `grove-explore-bin` compile mid-run before the 42/42 CLI
suite passed.

Cross-cutting concerns are handled well:

- **Backward compatibility:** `grove config` / `grove tap` remain as forwarding
  shims for one release, each printing the new spelling whether or not the
  `grove-explore` binary is present (AC2). This preserves the deprecation
  contract and avoids a hard break for existing users.
- **Config ownership (interim):** the config save path re-reads `mode` and
  `harnesses` from the on-disk `GroveConfig` before writing, so the guest
  writer never clobbers grove-owned fields (AC5). The guest-writer skew risk is
  covered by `config_round_trip_preserves_mode_and_harnesses`.
- **ADR-0002 alignment:** the mode-gated inert-render path (S03-T05) is removed
  as moot — the explore config is always live (AC4), and the string sweep drops
  the deprecated `.grove/explore.json` user-facing references (AC3).

## Deployment Notes

- Rides the next release train; no migration or regeneration required — old
  `grove config` / `grove tap` spellings keep working for one release.
- CHANGELOG should name the removal release for the shim deprecation window.
- No security scan required; no schema or deployment-topology changes.

## Follow-up Items (future sprints)

1. **Extract the save-path invariant into a shared helper.** The round-trip
   test re-implements the "rebuild GroveConfig from ExploreConfig + on-disk
   mode/harnesses" sequence inline rather than driving the real `event_loop`
   `Action::Save` arm. Extract a named helper both the event loop and test call
   so the invariant is tested at its true home and survives future save-path
   refactors.
2. **Migrate the first-run sentinel off `explore.json`.** `init.rs:136` still
   keys the first-run guard on `.grove/explore.json`. Pre-existing and
   acceptable during the one-release deprecation window; migrate to a
   `config.json`-based sentinel when `explore.json` support is dropped.
3. **Shim removal:** schedule deletion of the `grove config` / `grove tap`
   forwarding shims for the named removal release.
