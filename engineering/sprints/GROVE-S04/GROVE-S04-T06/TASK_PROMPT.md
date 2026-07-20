# GROVE-S04-T06: `init --as mcp-llm` funnel — shell-out to `grove-explore config` + graceful PATH degrade

**Sprint:** GROVE-S04
**Estimate:** M
**Pipeline:** default

---

## Objective

Keep the adoption funnel (VISION §6.4.1 — availability ≠ adoption) working
across the split: `grove init --as mcp-llm` registers both servers, writes the
two-surface steering, and reaches the first-run config TUI by **shelling out to
`grove-explore config`** — degrading gracefully when the sibling binary is not
on PATH instead of failing the init.

## Acceptance Criteria

1. `init --as mcp-llm` registers both servers and writes the two-surface
   steering (T04's `reconcile_harness` output — this task wires the init flow,
   not the harness content).
2. The first-run config TUI is no longer compiled into `grove init`: when
   explore setup is needed, `init` shells out to `grove-explore config`
   (inheriting the TTY), preserving today's first-run UX.
3. When `grove-explore` is **not** on PATH, `init` degrades gracefully: it
   still registers the server(s), completes successfully (exit 0), and prints
   "run `grove-explore config` to finish setup". This degrade path has an
   explicit test (PATH-controlled).
4. Non-TTY behavior is preserved: where the S03 flow failed fast or skipped the
   TUI, the new shell-out path makes the same choice (no hang, no broken
   terminal).
5. `--dry-run` performs no shell-out and no writes, as today.
6. Existing `init --as mcp|skill|both|grammars` behavior is unchanged;
   `tests/cli.rs` init assertions pass (updated only for the shell-out /
   degrade paths).
7. Workspace green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean build, files end with a newline.

## Context

Implements item 6 of `SPRINT_REQUIREMENTS.md`; ADR 0004 Stage 2 (the one
cross-binary seam). Depends on **T04** (register-both semantics) and **T05**
(`grove-explore config` exists). The ADR names this degrade path as one of the
two accepted interim couplings that must be tested. In the interim single-repo
form both binaries ship together, so the sibling is normally present — absence
is the edge (e.g. a user who installed only the `grove` binary via curl).

## Artifacts Involved

- `cli/src/init.rs` — TUI shell-out, PATH detection, degrade message, dry-run
  guard
- `tests/cli.rs` — degrade-path test (controlled PATH), shell-out skip in
  non-TTY, dry-run assertion

## Operational Impact

- **Version bump:** rides the next release train.
- **Regeneration:** none — funnel UX preserved for full installs.
- **Security scan:** not required (shell-out is to a fixed sibling binary name,
  no user-controlled arguments).
