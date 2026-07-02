# ARCHITECT_APPROVAL — GROVE-S02-T04

**Verdict:** Approved

## Rationale

Health-gated explore mode for `grove serve` is architecturally sound and consistent
with the grove stack (hand-rolled JSON-RPC 2.0 MCP server over stdio, no async
runtime, no external SDK).

- **Separation of concerns preserved.** `cli/src/mcp.rs` stays thin — it selects the
  surface and formats results; `core::explore` (`run_explore`, `health_probe`,
  `OpenAiCompatClient`, `ExploreConfig`, `ExploreError`) owns all execution. This
  respects the CLI/core boundary the project already enforces.
- **Startup gate (D3) is the right shape.** Probing health once before the request
  loop and binding the surface for the session avoids per-request probing cost and
  gives deterministic behavior. Precedence `force_standard > (force_explore | config)
  > health_probe` is coherent and testable.
- **Fail-safe posture.** Unhealthy provider or config load-failure transparently
  falls back to the standard 7-tool structural surface with a one-line stderr note;
  `initialize` always answers — never a dead server. Stdout stays protocol-only,
  stderr diagnostics-only. Mid-session `ProviderDown` returns `isError: true` with an
  actionable message rather than crashing.
- **Backward compatibility is a gating criterion and is met.** Default mode is
  byte-identical: the Standard branch reuses `tool_specs()`/`instructions()`/
  `call_tool()` verbatim; existing unit tests only threaded the unavoidable
  `&Surface::Standard` argument with assertions unchanged.
- **Gates green, independently confirmed by code-review and validation:** 178 tests
  pass (126 core + 32 cli-unit + 19 integration + 1 doc), clippy `--all-targets
  -D warnings` clean, no "fastcontext" string in cli/src or core/src.

## Cross-cutting concerns

- No shared-module impact. The change is confined to `cli/src/main.rs` (Serve variant
  + flags) and `cli/src/mcp.rs` (Surface enum + branching). The `explore` tool schema
  is a plain object (no top-level anyOf/oneOf), conforming to the MCP tool-schema
  checklist and keeping client compatibility across protocol versions 2025-06-18 /
  2025-03-26 / 2024-11-05.

## Deployment notes

- **Version bump:** not required (sprint-final; additive only).
- **Regeneration:** none for existing users — explore mode activates only via
  `.grove/explore.json` or an explicit `--explore` flag.
- **Backward compat:** default-mode byte-identical behavior verified (AC-5).

## Follow-up items for future sprints

- Add direct unit guards for the two paths currently exercised only transitively:
  the missing-`question` → `isError` branch (AC-3), and an assertion that
  `explore_tool_spec()` has no top-level anyOf/oneOf (locks AC-2 schema shape).
- Consider a unit test for `explore_instructions` model/base_url interpolation
  (low risk, currently untested).
