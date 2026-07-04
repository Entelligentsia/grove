# GROVE-S03-T03 — Architect Approval

**Verdict:** Approved

## Rationale

The task fixes bug-1 (sticky explore surface) by replacing the `explore.json`
file-existence sniff in `determine_surface` with a shared
`active_mode(root, ModeChoice) -> Mode` resolver in `core::config` that keys off
the declared `config.mode`. This is the architecturally correct move: surface
selection now derives from a single declared source of truth rather than an
incidental file artifact, and the resolver is shared between `serve` and
`doctor` so the two cannot diverge.

- Precedence is preserved and explicit: `ForceStandard → Mcp`, `ForceExplore →
  McpLlm`, `None → GroveConfig::load(root).mode` (Mcp on load failure).
  `--standard` wins over `--explore`, matching prior behavior.
- `determine_surface` now branches on `active_mode != Mode::McpLlm → Standard`
  as an explicit catch-all, so future `Mode` variants route safely to Standard
  rather than misrouting — a good defensive choice flagged in plan review and
  honored in implementation.
- Health-gated fallback (`mcp-llm` + unhealthy provider → standard) is preserved
  unchanged; the `health_probe` path remains intact.
- All 6 `active_mode` unit tests, the bug1 integration test (asserts exactly 7
  tools and absence of the `explore` tool), and the migrated-fixture unhealthy
  test pass. 268 tests green, clippy `-D warnings` clean, release build clean.

## Cross-cutting concerns

None. The change is contained to `core/src/config.rs` (new `ModeChoice` enum +
`active_mode` fn + re-exports) and `cli/src/mcp.rs` (`determine_surface`
rewrite). The double `GroveConfig::load` (resolver + explore-section read) is
benign and documented; the second load reads the freshly migrated config.json
and agrees on mode with no double-migration.

## Deployment notes

- No migrations, no config schema changes, no deployment topology impact.
- Single-binary distribution is unaffected.
- The plan correctly flags this as a **material** change (serve surface
  selection behavior). A **version bump is required at release** per the stack's
  release conventions.

## Follow-ups for future sprints

- Non-blocking: the AM-1 unit-test fixture JSON has a stray trailing quote making
  config.json unparseable. The test still passes because `ForceStandard`
  short-circuits before the config read, but the fixture no longer demonstrates
  its stated intent. Worth a cleanup pass.
- Advisory: `active_mode` is not side-effect free on the `None` legacy path —
  `GroveConfig::load` may trigger migration + a config.json write, and `doctor`
  will migrate too. Consistent with `load()` semantics but a doc comment on
  `active_mode` clarifying this would help future readers.
