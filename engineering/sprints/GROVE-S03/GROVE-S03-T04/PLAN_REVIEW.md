# PLAN_REVIEW — GROVE-S03-T04 (standalone review)

**Verdict:** Approved

## Scope reviewed
`reconcile_harness` refactor + 20-pair transition-matrix test in `cli/src/init.rs`,
`run` loading/persisting `mode` via `GroveConfig`, and the strip helpers. Verified
independently against the actual source (`cli/src/init.rs`, `core/src/config.rs`,
`core/src/explore/config.rs`, `cli/tests/cli.rs`), not the plan's self-report.

## Correctness / feasibility — confirmed
1. All referenced APIs exist and are reachable from the CLI crate:
   - `grove_core::config::Mode` derives `Debug, Clone, Copy, PartialEq, Eq` — so
     `for &old in &modes` and `if old == new` in the matrix test compile.
   - `GroveConfig::load` (Result), `GroveConfig::save`, `active_mode`, `ModeChoice`
     are `pub` and already imported in `cli/src/mcp.rs`.
   - `ExploreConfig::load` / `ExploreConfig::config_path` exist
     (`core/src/explore/config.rs:180,189`) — the step-6 explore round-trip is viable.
2. Current `init.rs` matches the plan's "Analyse existing code paths" table exactly:
   `write_harness`, `write_steering_md` (splice-in-place / append / create),
   `write_mcp_json_with`, sentinel constants, and the per-mode content builders.
3. The harness table (Mcp/Both→`[serve]`, McpLlm→`[serve,--explore]`, Skill/Grammars→
   strip; CLAUDE.md present except Grammars; AGENTS.md present only McpLlm) is a
   faithful, mode-exact mapping that satisfies AC#1 and the bug-2 fix (AC#3).
4. `run` change is safe: dry-run still short-circuits via `provision_project`'s empty
   return before reconcile/save, so AC#6 dry-run behaviour is preserved.

## Integration-test impact — confirmed non-breaking
`init_provisions_and_wires_harness_per_target` (cli/tests/cli.rs:340) asserts only
file presence and `wrote`-order (`.mcp.json` < `CLAUDE.md` < `grove.lock`) and
dry-run emptiness. It never asserts the *absence* of `.grove/config.json`, so the
new config write does not break it. `reconcile_harness` preserves the reported-write
order for Mcp (`.mcp.json` then `CLAUDE.md`), which keeps that assertion green — the
implementer must preserve this ordering.

## Advisory notes (non-blocking)
1. **AC#5 active_mode + T07 fixture reuse.** AC#5 literally asks the matrix to also
   assert "the surface `serve` would boot (via `active_mode`) [is] consistent with B"
   and that fixtures be "structured so T07's harness-consistency check can reuse them."
   The plan (correctly) notes `reconcile_harness` alone cannot drive `config.json`, and
   defers the `active_mode` check to the single-pass `reconcile_harness_then_save_config_active_mode`
   test. That is a defensible split, but the deferred test covers one round-trip, not
   all 20. Recommend either (a) extending the config-save round-trip inside the matrix
   loop, or (b) a one-line note in the code/PLAN explaining the deferral and how the
   matrix fixtures are shaped for T07 reuse, so the AC#5 intent is demonstrably met.
2. **Strip whitespace idempotency.** `write_steering_md`'s append path inserts a
   `\n\n` separator before `CLAUDE_START`. `strip_steering_block` must remove that
   introduced blank-line separator (not just the sentinel span) so repeated A→B→A
   cycles don't accrete blank lines. The 20-pair matrix is single-hop and won't catch
   cycle growth — worth an explicit assertion or a comment that strip normalizes
   surrounding whitespace. The plan mentions "including surrounding blank lines" — good;
   just ensure the test/impl actually enforces it.
3. **EOF newline after strip (AC#7).** When host content precedes a stripped block,
   ensure the written result still ends in a single trailing newline. The block-only
   case correctly yields an empty file (not a deletion) — good call.
4. **Unused `old_mode` param.** Keeping it per AC#1's mandated signature is correct;
   since `reconcile_harness` is a private `fn`, an unused named parameter will not trip
   `clippy -D warnings` (no rename to `_old_mode` needed).

None of these require a revision loop — they are refinements the implementer should
fold in. The plan is complete, feasible, and host-content-safe by design.
