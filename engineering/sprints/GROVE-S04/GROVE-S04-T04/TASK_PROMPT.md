# GROVE-S04-T04: `mcp-llm` = register-both — `reconcile_harness` both-servers column, forward migration, extended transition-matrix

**Sprint:** GROVE-S04
**Estimate:** L
**Pipeline:** default

---

## Objective

Re-key the `mcp-llm` mode's meaning per ADR 0004's amendment to ADR 0002 §1:
with no surface left to select, `mcp-llm` now means "register **both** MCP
servers in `.mcp.json` and write the delegation steering." Extend
`reconcile_harness`'s mode→files matrix with the both-servers column, migrate
existing single-registration `mcp-llm` projects forward, and extend the
transition-matrix test (sprint gate test b).

## Acceptance Criteria

1. `init --as mcp-llm` registers **both** server entries in `.mcp.json` (the
   structural `grove serve` entry and the `grove-explore` entry) and writes
   steering that names both surfaces and when to use each (delegate broad
   "where is X" sweeps to `explore`; use `map`/`source`/etc. directly for
   precision work). Steering stays locator-framed with the recommended flow.
2. Every mode's harness output converges through the single `reconcile_harness`
   writer (ADR 0002 design intact). Leaving `mcp-llm` strips the
   `grove-explore` registration and its steering block; only sentinel-delimited
   grove-authored blocks and grove's own server entries are touched.
3. **Forward migration:** a pre-split `mcp-llm` project (`.mcp.json` with the
   single `["serve","--explore"]`-style registration) is converged to the
   two-registration layout on the first `init` run — no silently broken
   registration survives. Covered by an explicit test.
4. **Gate test (b):** the S03-T04 transition-matrix test is extended — every
   ordered `A → B` mode switch across `{mcp, skill, both, mcp-llm, grammars}`
   leaves `.mcp.json` (both entries), `CLAUDE.md`/`AGENTS.md` steering, and the
   served surfaces mutually consistent (both surfaces are now constants, so
   consistency = registrations and steering match the mode).
5. `Mode::LEGAL` is unchanged; no mode or tunable added/removed.
6. Host-authored content in `CLAUDE.md`/`AGENTS.md`/other `.mcp.json` servers
   is preserved verbatim (existing host-content test extended to the new
   column).
7. Workspace green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean build, files end with a newline.

## Context

Implements item 4 of `SPRINT_REQUIREMENTS.md`; ADR 0004 §Stage 2 (mode
amendment). Depends on **T02** (the server identity being registered) and
**T03** (no `--explore` arg left to write). Builds directly on S03-T04's
`reconcile_harness` + 20-pair matrix test — extend, don't rewrite. This task
owns the sprint's highest-likelihood risk (existing `mcp-llm` projects breaking
on upgrade); the migration AC is the mitigation. Memory note applies: the
delegation steering must be locator-framed and carry the recommended flow, or
the outer agent bypasses grove.

## Artifacts Involved

- `cli/src/init.rs` — `reconcile_harness` both-servers column, migration,
  steering content
- `core/src/harness.rs` — per-agent registration shapes for the second server
- `cli/src/init.rs` tests + `tests/cli.rs` — extended matrix, migration,
  host-content tests

## Operational Impact

- **Version bump:** rides the next release train.
- **Regeneration:** existing `mcp-llm` users run `grove init --as mcp-llm` once
  (or any `init`) to converge; doctor (T07) flags the stale layout meanwhile.
- **Security scan:** not required.
