# GROVE-S02-T04: `grove serve` explore mode — exclusive tool surface + health-gated startup/shutdown

**Sprint:** GROVE-S02
**Estimate:** M
**Pipeline:** default

---

## Objective

Give `grove serve` its second face: in explore mode the MCP server surfaces
**only** `explore` (client-side `mcp__grove__explore`) — the 7 structural tools
disappear from the outer agent's view — and the surface is health-gated: no
healthy provider at startup, no server; provider connection error mid-session,
clean shutdown.

## Acceptance Criteria

1. `grove serve` selects explore mode when the project has `.grove/explore.json`
   (T01) or an explicit CLI flag forces it; an explicit flag can also force
   standard mode despite a config being present. Flag naming follows existing
   clap conventions in `main.rs`; `cli/src/mcp.rs` stays thin — mode logic
   formats, `core::explore` executes.
2. In explore mode, `tools/list` returns **exactly one** tool, `explore`:
   plain `{type: object, properties, required}` schema (question: string,
   required), per the MCP checklist (no top-level anyOf/oneOf). The server
   `instructions()` string describes delegation, not the structural tools.
3. `tools/call` on `explore` runs `core::explore::run_explore` with the real
   `OpenAiCompatClient`; tool errors (bad args, explore failures other than
   provider-down) return `isError: true` results, not JSON-RPC errors.
4. **Startup gate:** explore mode runs `health_probe` before serving; on
   failure the process exits non-zero with a descriptive stderr message
   (which probe failed: unreachable vs model-missing) and never answers
   `initialize`. Stdout carries protocol only, stderr diagnostics only.
5. **Runtime shutdown (D3):** `ExploreError::ProviderDown` during a call →
   the server writes a descriptive stderr message and exits; no half-alive
   erroring state.
6. **Default mode is byte-identical:** without explore config/flag, `serve`
   behavior is unchanged — all existing `cli/src/mcp.rs` unit tests and
   `tests/cli.rs` MCP cases pass unmodified, and the README's MCP smoke test
   still lists the 7 structural tools.
7. New integration test (deterministic, no provider needed): explore-mode
   serve with an unreachable `base_url` exits non-zero fast with the
   descriptive message and emits no protocol output.
8. Warning-clean, clippy clean, tests green; no "fastcontext" string.

## Context

Depends on **T03** (`run_explore`, `ExploreError::ProviderDown`). Decisions D3
(startup probe; connection errors shut down the server) and D5 (exclusive
surface; `grove serve` gains a mode). The structural ops remain available
*internally* to the inner explorer — exclusivity is about what the *outer*
agent sees. Mind the stack checklist: protocol versions `2025-06-18`,
`2025-03-26`, `2024-11-05` must all still initialize in both modes.

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
  criterion (criterion 6).
