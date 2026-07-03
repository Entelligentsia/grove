# ARCHITECT_APPROVAL — GROVE-S02-T06

## `grove init --as mcp-llm` — explore-mode harness provisioning

**Verdict:** Approved

## Rationale

The change adds a new `Target::McpLlm` variant to `grove init --as`, wiring
explore-mode `.mcp.json` (`serve --explore`), idempotent `CLAUDE.md`/`AGENTS.md`
sentinel steering blocks, a non-TTY first-run guard, dry-run output, and
first-run TUI delegation.

Architecturally this is a clean, additive CLI change that sits squarely within
the established stack:

- **Consistent with clap ValueEnum pattern** — `Target::McpLlm` auto-produces
  the `--as mcp-llm` surface and `--help` entry; no bespoke arg plumbing.
- **Reuses proven idioms** — the read/merge `.mcp.json` write pattern, the
  `grove:start/end` sentinel idempotency, and the T05 `IsTerminal` guard are all
  existing, load-bearing patterns; the new code composes them rather than
  inventing parallel machinery.
- **Existing targets byte-untouched** — the `write_harness` McpLlm early-return
  isolates the new path; `writes_mcp()` remains `false` for McpLlm so the generic
  ready block never double-prints. All 19 pre-existing unit tests remain green.
- **MCP alignment** — `serve --explore` is registered exactly as the stdio MCP
  server contract expects; no protocol surface change.

Code review and validation both returned **approved** (200 tests pass, clippy
`-D warnings` clean, no stray `fastcontext` string).

## Deployment Notes

- No migrations, no schema changes, no deployment-topology impact.
- Single-binary distribution unaffected — this is a compile-time CLI addition.
- User-facing: a new `--as mcp-llm` option; existing `mcp|skill|both` flows are
  unchanged and backward-compatible.
- Docs updated (README.md, docs/setup.md) so the new target is discoverable.

## Follow-up Items (future sprints)

1. **Sentinel constant naming** — `agents_section`/`claude_section` share the
   `CLAUDE_START/END` constants for AGENTS.md. A cosmetic rename to neutral
   `GROVE_START/END` would read cleaner now that they span two harness files.
2. **AC2(b) wording reconciliation** — the AC says "pre-populated if the file
   exists"; the implementation correctly *skips* the TUI on re-runs. Reconcile
   the AC text if it is ever revisited (behaviour is correct as shipped).
