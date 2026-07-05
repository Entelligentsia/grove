# ARCHITECT_APPROVAL — GROVE-S03-T04

**Verdict:** Approved

## Task
`reconcile_harness(root, old_mode, new_mode)` — single harness writer with strip
direction, config.json mode persistence, and a 20-pair transition-matrix test.

## Architectural Rationale
- **Coherence:** Consolidating all mode→harness mapping into one `reconcile_harness`
  writer eliminates the drift class where switching modes left `old_mode` residue
  on disk (bug 2). Driving every file purely from `new_mode` is the correct
  invariant — the on-disk harness becomes a pure function of declared mode, not
  of history. This is a strictly stronger, more testable design than the removed
  `write_harness`.
- **Blast radius contained:** Changes are localized to `cli/src/init.rs`. No changes
  to `core/src/config.rs`; the CLI `Target` enum bridges to the store `Mode` enum
  via a 1-to-1 `to_mode()` mapping, keeping the existing config surface stable.
- **Safety of destructive ops:** Stripping is sentinel-bounded
  (`<!-- grove:start -->…<!-- grove:end -->`) and the `.mcp.json` edit removes only
  the `mcpServers.grove` key. Host-authored content and sibling MCP servers are
  preserved verbatim, proven by `reconcile_harness_preserves_host_content`. This
  respects the harness's shared-file posture (CLAUDE.md/AGENTS.md/.mcp.json are
  host-owned artifacts grove co-tenants).
- **Test rigor:** The table-driven 20-pair (A→B) matrix across all 5 modes gives
  full transition coverage; fixtures are structured for T07 reuse. `active_mode`
  round-trip is covered separately per the approved plan.

## Deployment Notes
- **Version bump required at release** — init now reconciles + strips harness files
  (the bug-2 fix); observable behaviour change on mode switches.
- **No migration required.** Users switching modes get correct cleanup automatically.
  Already-drifted projects self-heal with a one-time `grove init --as <current-mode>`.
- Dry-run safety confirmed: `provisioned.is_empty()` guard short-circuits before
  `reconcile_harness`/`save`, so dry-run never mutates harness or config.json.
- Non-TTY guard correctly widened to treat a config.json-declared `mcp-llm` as
  already-configured. No security scan required.

## Follow-up Items (future sprints)
- **T07** should consume the matrix fixtures for its harness-consistency check;
  fixture helpers are currently private to `mod tests` and will need a visibility
  change or re-impl (flagged non-blocking by plan review + validation).
- Non-blocking wording: `reconcile_harness` doc says "drives from on-disk state"
  but drives purely from `new_mode` — tidy the comment when next touching the file.
- The matrix is single-hop; if repeated A→B→A cycles ever matter, add a multi-hop
  idempotency assertion to guard against blank-line accretion.
