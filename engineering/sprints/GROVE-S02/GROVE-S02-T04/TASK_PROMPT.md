# GROVE-S02-T04: `grove serve` explore mode — exclusive tool surface + health-gated startup/shutdown

**Sprint:** GROVE-S02
**Estimate:** M
**Pipeline:** default

---

## Objective

Give `grove serve` its second face: in explore mode, **when the provider is
healthy**, the MCP server surfaces **only** `explore` (client-side
`mcp__grove__explore`) — the 7 structural tools disappear from the outer
agent's view. The surface is health-gated at startup, but the server is always
useful: if the provider is unhealthy at startup, `serve` **falls back to the
standard 7-tool structural surface** (identical to `--as mcp`) rather than
dying. A mid-session provider loss returns a recoverable tool error rather than
crashing the server.

## Acceptance Criteria

1. `grove serve` selects explore mode when the project has `.grove/explore.json`
   (T01) or an explicit CLI flag forces it; an explicit flag can also force
   standard mode despite a config being present. Flag naming follows existing
   clap conventions in `main.rs`; `cli/src/mcp.rs` stays thin — mode logic
   formats, `core::explore` executes.
2. **Startup gate decides the surface (D3):** explore mode runs `health_probe`
   once before serving.
   - **Healthy** → `tools/list` returns **exactly one** tool, `explore`: plain
     `{type: object, properties, required}` schema (question: string, required),
     per the MCP checklist (no top-level anyOf/oneOf). The server
     `instructions()` string describes delegation, not the structural tools.
   - **Unhealthy** (unreachable or model-missing) → the server **falls back to
     the standard 7-tool structural surface** — byte-identical to default
     `--as mcp` mode (`tools/list`, `instructions()`, and `tools/call` all
     behave as today) — and writes a one-line descriptive stderr note that it
     fell back and why. The server always answers `initialize`; it is never a
     dead server. Stdout carries protocol only, stderr diagnostics only.
3. `tools/call` on `explore` (healthy path) runs `core::explore::run_explore`
   with the real `OpenAiCompatClient`; tool errors (bad args, explore failures
   including mid-session provider-down) return `isError: true` results, not
   JSON-RPC errors.
4. **Mid-session provider loss:** `ExploreError::ProviderDown` during an
   `explore` call → the call returns `isError: true` with an actionable message
   (provider down; check the endpoint / run `grove config` / restart grove to
   pick up the structural fallback). The server does **not** crash or exit.
5. **Default mode is byte-identical:** without explore config/flag, `serve`
   behavior is unchanged — all existing `cli/src/mcp.rs` unit tests and
   `tests/cli.rs` MCP cases pass unmodified, and the README's MCP smoke test
   still lists the 7 structural tools.
6. New integration test (deterministic, no provider needed): explore-mode
   serve with an unreachable `base_url` comes up and `tools/list` returns the
   **7 structural tools** (fallback), with the stderr fallback note present;
   `initialize` succeeds.
7. Warning-clean, clippy clean, tests green; no "fastcontext" string.

## Context

Depends on **T03** (`run_explore`, `ExploreError::ProviderDown`). Decisions D3
(startup probe decides the surface: **healthy → explore-only, unhealthy →
fall back to the 7-tool structural surface**, never a dead server; mid-session
provider loss returns a recoverable `isError`, not a crash) and D5 (exclusive
explore surface when healthy; `grove serve` gains a mode). The unhealthy-startup
fallback reuses the *exact* existing default-mode serve path — so it is the
same code, not a re-implementation — which also makes criterion 5's
byte-identical guarantee cheap. The structural ops remain available
*internally* to the inner explorer in the healthy path. Mind the stack
checklist: protocol versions `2025-06-18`, `2025-03-26`, `2024-11-05` must all
still initialize in every mode.

## Artifacts Involved

- Edited: `cli/src/mcp.rs` (mode-aware `serve`, explore tool spec + call
  path), `cli/src/main.rs` (`Serve` args).
- Verify: existing `mcp.rs` inline tests, `tests/cli.rs` MCP smoke cases.
- New: explore-mode integration cases in `tests/cli.rs`.

## Operational Impact

- **Version bump:** not required (sprint-final).
- **Regeneration:** none for existing users; explore mode activates only via
  config/flag.
- **Backward compat:** default-mode byte-identical behavior is a gating
  criterion (criterion 5).
