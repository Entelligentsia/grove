# ARCHITECT_APPROVAL — GROVE-S03-T05: `config` TUI mode badge + inert explore-section rendering

**Verdict:** Approved

## Rationale

The implementation extends the grove `config` TUI to read the active integration
`mode` from `GroveConfig`, surface it as a read-only title badge, and render the
explore section inert/greyed (DIM borders, no-op edits, dormant notice) when
`mode != mcp-llm`. It is consistent with the project's architecture and honours
the load-bearing invariant of this sprint.

- **Elm-style layering preserved.** `grove_mode` and `explore_active` are pure
  model state; the inert gate lives entirely in `update`; presentation stays in
  `view`. No cross-layer leakage — the change respects the existing TUI seams.
- **Read-only mode, no second harness writer (AC4).** `grove_mode` is set once
  in `App::from_grove_config` and never mutated by any `Msg`. No mode-selection
  control was added; `Save` is reachable only under `McpLlm` and round-trips
  `mode` unchanged. The TUI cannot become a competing writer of harness state —
  the central design constraint is upheld.
- **All 6 ACs satisfied.** Badge covers all 5 `Mode` variants; explore section
  live under `McpLlm` and inert otherwise with a dormant note; edits and Save are
  blocked when inactive; new coverage asserts badge + inert behaviour; full suite
  green with clippy `-D warnings` clean.
- **Regression-safe.** `App::default()` routes through an `McpLlm`-active config,
  so the 17 pre-existing update tests and the broader suite (283 passed / 0
  failed) remain green without modification.

## Deployment Notes

- **Version bump:** required at release — this is a visible TUI change.
- **Migrations / regeneration:** none. No schema, lockfile, or grammar changes.
- **Backward compatibility:** `main.rs` and `init.rs` load `GroveConfig` with a
  graceful fallback to legacy `ExploreConfig::load`, so pre-migration projects
  without a `GroveConfig` on disk continue to function.
- **Security scan:** not required — no new dependencies, no I/O surface change.

## Follow-up Items (future sprints)

- Advisory (non-blocking): field body text is not additionally dimmed beyond the
  DIM borders + dormant notice + footer hint. Current treatment satisfies AC3;
  consider fuller body-text dimming if visual clarity feedback warrants it.
- Once all downstream consumers migrate to `GroveConfig`, the legacy
  `ExploreConfig::load` fallback in `main.rs`/`init.rs` can be retired to reduce
  the dual-read surface.
