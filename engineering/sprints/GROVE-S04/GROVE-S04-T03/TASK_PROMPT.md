# GROVE-S04-T03: `grove serve` always structural — delete `Surface`/`determine_surface`/serve mode flags/health fallback

**Sprint:** GROVE-S04
**Estimate:** M
**Pipeline:** default

---

## Objective

Make `grove serve`'s surface a constant. Delete the bimodal machinery —
`Surface`, `determine_surface`, the `serve --explore`/`--standard` flags, and
the silent health-probe fallback — so `grove serve` unconditionally serves the
7-tool structural surface and `cli/src/mcp.rs` carries no explore knowledge.
This structurally eliminates the steering-vs-served-surface mismatch class
(Bug-1's family).

## Acceptance Criteria

1. `Surface`, `determine_surface`, and the health-probe fallback are deleted
   from `cli/src/mcp.rs`; the file has zero `explore` imports (from core or the
   new crate).
2. The `serve --explore` and `--standard` flags are removed from the CLI. An
   invocation using them produces a clear error naming the replacement
   (`grove-explore` / plain `grove serve`) — not a silent ignore.
3. `grove serve` boots the 7 structural tools regardless of declared mode,
   config contents, or provider health — asserted by an integration test that
   sets `mode: mcp-llm` in `.grove/config.json` and verifies `tools/list`
   returns the 7 structural tools.
4. ratatui/crossterm are no longer dependencies of the `grove` binary
   (`cargo tree` check; the TUI code itself moves in T05 — this task covers the
   serve-path decoupling and must be consistent with T05's move).
5. Existing structural-surface integration tests in `tests/cli.rs` pass
   unchanged except where they exercised the deleted flags/fallback (those are
   updated to assert the new constant-surface behavior).
6. Workspace green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean build, files end with a newline.

## Context

Implements item 3 of `SPRINT_REQUIREMENTS.md`; ADR 0004 Stage 2 (second half),
amending ADR 0002 §1 (`determine_surface` is deleted; `mode` no longer selects
a surface). Depends on **T01**. Runs in parallel with **T02** — T02 builds the
replacement home for the explore surface; this task removes the old one. Note
the S03-T03 doc comment on `determine_surface` documents Bug-1; preserve that
history in the commit message, not the code. The `active_mode` resolver from
S03-T03 loses its `serve` consumer here but is still used by `init`/`doctor`
until T04/T07 rework their keying — do not delete it in this task.

## Artifacts Involved

- `cli/src/mcp.rs` — `Surface`/`determine_surface`/fallback deletion
- `cli/src/main.rs` — serve flag removal + replacement-naming error
- `cli/Cargo.toml` — dependency prune (with T05)
- `tests/cli.rs` — updated serve-surface assertions

## Operational Impact

- **Version bump:** rides the next release train; the removed flags need a
  CHANGELOG deprecation note.
- **Regeneration:** existing `mcp-llm` projects keep working only after T04's
  migration — sequencing inside the sprint means the release ships T03+T04
  together; the intermediate state never ships alone.
- **Security scan:** not required.
