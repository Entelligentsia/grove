# GROVE-S03-T04: `reconcile_harness` — single harness writer + transition-matrix test

**Sprint:** GROVE-S03
**Estimate:** L
**Pipeline:** default

---

## Objective

Make `grove init --as <mode>` converge **every** harness file to the declared
mode from one code path, cleaning up the prior mode's residue — fixing **bug 2**
(the orphaned `AGENTS.md`) and closing the whole `A→B` transition-bug class.
Extract a single `reconcile_harness` function that owns the `mode → on-disk
harness` mapping, and prove it with a transition-matrix test over all mode
switches.

## Acceptance Criteria

1. `reconcile_harness(root, old_mode: Option<Mode>, new_mode: Mode) ->
   Result<Vec<String>>` writes, rewrites, or **strips** `.mcp.json`,
   `CLAUDE.md`, and `AGENTS.md` so the on-disk harness matches `new_mode`
   exactly, removing `old_mode`'s residue.
2. `grove init --as <mode>` reads the prior `mode` from `config.json` (if any),
   calls `reconcile_harness(root, old, new)`, then persists the new `mode` to
   `config.json`. `mode` is recorded verbatim on every run.
3. Leaving `mcp-llm` **removes** the grove-authored explore block from
   `AGENTS.md` (bug 2 fixed) and drops the `--explore` arg from `.mcp.json`
   (reverts it to `["serve"]` or removes grove's entry per target mode).
4. Only grove-authored, **sentinel-delimited** blocks and grove's own
   `.mcp.json` entry are ever written/stripped; host-authored content in
   `CLAUDE.md` / `AGENTS.md` / other `.mcp.json` servers is preserved verbatim
   (explicit test).
5. A **table-driven transition-matrix test** enumerates every ordered `A→B`
   switch across `{mcp, skill, both, mcp-llm, grammars}` and asserts that after
   `init --as B` the resulting `.mcp.json` args, `CLAUDE.md` block, `AGENTS.md`
   block, and the surface `serve` would boot (via `active_mode`) are mutually
   consistent with `B`. Fixtures are structured so **T07**'s harness-consistency
   check can reuse them.
6. Existing `init --as mcp|skill|both|grammars|mcp-llm` behaviour is otherwise
   unchanged; current `tests/cli.rs` init assertions pass (updated only where
   they must reflect the new `config.json` write / reconciliation).
7. `cargo build` warning-clean, `cargo clippy -- -D warnings` clean, `cargo
   test` green. Files end with a newline.

## Context

Implements item 4 of `SPRINT_REQUIREMENTS.md` and ADR 0002 §Decision-2. Current
code to refactor: `cli/src/init.rs` — `write_harness`, `write_mcp_json[_with]`,
`write_mcp_json_explore`, `write_claude_md`, `write_agents_md`, `claude_section`,
`agents_section`, and the `Target` enum. The sentinel-block idempotency approach
already exists (`write_claude_md`/`write_agents_md`); this task adds the
**strip** direction. Depends on **T01** (persist `mode`) and **T03**
(`active_mode`, so the matrix test can assert surface consistency). This is the
riskiest task — host-content safety is a hard requirement.

## Artifacts Involved

- `cli/src/init.rs` — new `reconcile_harness`; refactor the per-file writers to
  support write **and** strip; `run` reads prior mode, reconciles, persists.
- `core/src/config.rs` — `GroveConfig` read/write of `mode` (from T01);
  `active_mode` (from T03) used by the test.

## Operational Impact

- **Version bump:** required at release (init now reconciles + strips harness
  files — the bug-2 fix).
- **Regeneration:** users switching modes get correct cleanup automatically;
  a one-time `grove init --as <current-mode>` reconciles any already-drifted
  project.
- **Security scan:** not required.
