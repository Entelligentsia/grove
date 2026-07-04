# Architect Approval — GROVE-S03-T02

**Verdict:** Approved

## Summary

Legacy `.grove/explore.json` → `.grove/config.json` forward-migration on
`GroveConfig::load`. The implementation extends `load` into a three-branch
cascade (config.json → normal load; explore.json → migrate; neither → `grove init`
steer) and satisfies all six acceptance criteria with tests 5a–5d green.

## Architectural Assessment

- **Coherence with T01.** The migration reuses the established T01 conventions:
  a private `LegacyExploreRaw` wire struct mirroring the pre-S03 `mode` key,
  validating `from_name` remaps, and the existing atomic `GroveConfig::save`.
  No new persistence mechanism, no parallel config path — the on-disk contract
  stays singular (`config.json` is the one source of truth after first load).
- **Blast radius is contained.** The only cross-module surface change is widening
  `Provider::from_name` / `Steering::from_name` to `pub(crate)` in
  `explore/config.rs`, plus a `pub(crate)` `DEPRECATION_WARNING` const. Both are
  crate-internal; no public API surface shifts, so downstream MCP/CLI consumers
  are unaffected.
- **Idempotency is structural, not incidental.** Post-migration, `config.json`
  shadows the migration branch, so the deprecation warning fires exactly once.
  Test 5b's mtime assertion guards against inadvertent rewrites of the legacy
  file — the migration reads explore.json but never mutates or deletes it, which
  correctly defers cleanup to the future `grove doctor` task.
- **Data preservation verified.** `explore.mode` → `explore.steering` remap plus
  provider/base_url/model/allowed_tools/tap/trace_retain all carried forward with
  no loss (AC2).

## Deployment Notes

- **Version bump required at release.** This changes on-disk behaviour for every
  deployed `mcp-llm` project: `config.json` is synthesised on the next
  `serve`/`load`. Migration is automatic; no user action required. The one-time
  stderr deprecation warning informs the operator to commit `config.json` and
  remove `explore.json` from the repo.
- No security scan required (no new I/O sink, no network, no secret surface).

## Follow-up Items (future sprints)

- `grove doctor` should warn on the continued presence of a stale
  `.grove/explore.json` after migration (cleanup is intentionally deferred here).
- Consider capturing the deprecation warning via stderr in a future test harness;
  5d currently asserts on the `DEPRECATION_WARNING` const content — an accepted,
  documented tradeoff.
- Malformed `explore.json` yields `Err` rather than the `grove init` steer —
  intentional and documented; revisit if telemetry shows user confusion.
