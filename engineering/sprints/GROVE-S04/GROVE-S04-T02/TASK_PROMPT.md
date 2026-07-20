# GROVE-S04-T02: `grove-explore` binary — own MCP server identity, explore tool only, startup health gate

**Sprint:** GROVE-S04
**Estimate:** M
**Pipeline:** default

---

## Objective

Give the locator product its own face: a `grove-explore` bin target that runs
an MCP server exposing **only** the `explore` tool under its own server name,
linking `grove-cst` for in-process structural ops. Provider-down becomes a
visible startup failure of *this* server — never a silent morph into a
different surface.

## Acceptance Criteria

1. A `grove-explore` binary exists in the workspace and serves an MCP server
   (stdio, newline-delimited JSON-RPC 2.0, same protocol versions as `grove
   serve`) exposing exactly one tool: `explore`. Tool name, input schema, and
   result contract are unchanged from today's explore surface.
2. An unhealthy provider at startup is a **startup error**: actionable message
   (reusing the existing `health_probe` fix hints) on stderr and a non-zero
   exit. The server never starts with a different or empty surface.
3. Mid-session provider loss returns today's recoverable `isError: true` tool
   result (behavior preserved from the current explore surface).
4. The inner explorer reaches grove ops as direct in-process Rust calls (no MCP
   hop, no subprocess) — the delegate's economics are untouched.
5. The binary reads its config from the `.grove/config.json` explore section
   exactly as the current explore surface does (ADR 0002 placement; no config
   file move — that is stage 3).
6. Integration coverage: an MCP smoke test (initialize + tools/list) asserts
   the single-tool surface and the server identity; a startup-failure test
   covers the unhealthy-provider path.
7. Workspace green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean build, files end with a newline.

## Context

Implements item 2 of `SPRINT_REQUIREMENTS.md`; ADR 0004 Stage 2 (first half).
Depends on **T01** (the explore crate exists). The current explore surface
lives in `cli/src/mcp.rs` behind `Surface::Explore` — this task creates its
permanent home; **T03** deletes the old one. Server naming: keep family
branding (`grove-explore`), consistent with the `grove-explore-*` model line.
The MCP protocol loop is hand-implemented (no SDK, no async runtime) — follow
`cli/src/mcp.rs`'s existing pattern. Decide bin placement (bin in `explore/`
crate vs. a thin new bin crate) in the plan phase; either must keep engine
logic out of the bin's main.

## Artifacts Involved

- New bin target (location per plan) — MCP loop, startup health gate, dispatch
  to `run_explore`
- `explore/` crate — consumed for the loop, config, and health types
- `tests/` — MCP smoke + startup-failure integration tests

## Operational Impact

- **Version bump:** rides the next release train (new binary first ships then).
- **Regeneration:** none yet — registration changes land in T04/T06.
- **Security scan:** not required.
