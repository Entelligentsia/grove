# CODE_REVIEW — GROVE-S03-T02 (standalone review)

**Verdict:** Approved

## Scope

Legacy `.grove/explore.json` → `.grove/config.json` migration on `GroveConfig::load`.
Reviewed the working-tree diff against `HEAD` (T02 is uncommitted; T01 is at `a445b72`)
for `core/src/config.rs` and `core/src/explore/config.rs`. Verified independently by
reading source, resolving symbol visibility, and running the test + clippy suites.

## Independent Verification

- **Build/tests:** `cargo test --release --locked -p grove-cst config` → 22 passed, 0 failed.
  All four new tests present and green: `migrate_legacy_explore_writes_config_json` (5a),
  `second_load_after_migration_reads_config_not_legacy` (5b),
  `config_json_present_ignores_stale_explore_json` (5c), `deprecation_warning_emitted` (5d).
- **Lint:** `cargo clippy -p grove-cst --all-targets -- -D warnings` → clean.
- **Newline (AC6):** both modified files end with a trailing newline.
- **Symbol visibility (prior advisory resolved):** `DEFAULT_TRACE_RETAIN` is `pub const`
  (config.rs:109) inside `pub mod config` — so `crate::explore::config::DEFAULT_TRACE_RETAIN`
  in `default_legacy_trace_retain()` compiles. `Provider::from_name` / `Steering::from_name`
  correctly promoted to `pub(crate)`; both re-exported via `crate::explore`.
- **`ExploreConfig::config_path`** returns `.grove/explore.json` (read-only reuse) — the
  migration reads the correct legacy path and never mutates it.

## Correctness — Acceptance Criteria

| AC | Assessment |
|----|-----------|
| 1 — migrate when config.json absent + explore.json present | ✅ Three-branch cascade in `load`: config.json → normal; else explore.json → `migrate_from_legacy_explore`; else `grove init` bail. |
| 2 — legacy `mode` → `steering` | ✅ `Steering::from_name(&raw.mode)` validates + remaps; provider/base_url/model/allowed_tools/tap/trace_retain preserved with no data loss. |
| 3 — one-time deprecation warning | ✅ `eprintln!(DEPRECATION_WARNING)` after atomic `save`; one-time is structurally guaranteed because branch 1 (config.json) shadows the migration on subsequent loads (5b confirms idempotency). |
| 4 — neither file → same actionable error | ✅ Bail path unchanged; existing `missing_file_actionable_error` still green. |
| 5 — four tests a–d | ✅ All present and passing. |
| 6 — build/clippy/test clean, trailing newline | ✅ Verified independently. |

## Quality / Architecture

- Follows the established T01 pattern precisely: a private `LegacyExploreRaw` wire struct
  with `String` enum keys, parsed through validating `from_name` calls — consistent with
  `RawGroveConfig` / `RawExploreConfig`.
- The named `DEPRECATION_WARNING` const is a clean, testable design (avoids fragile
  cross-platform stderr capture). Minimal `pub(crate)` surface. Atomic save reused, so the
  migration inherits temp-file+rename durability.
- `default_legacy_trace_retain()` correctly delegates to the shared `DEFAULT_TRACE_RETAIN`,
  matching the original serde defaults so an absent `trace_retain` migrates without drift.

## Advisory Notes (non-blocking)

1. **Test 5d does not capture stderr** — it asserts on the const's content and proves the
   migration branch executed via the config.json side-effect. This is the agreed tradeoff
   from plan-review; acceptable. A future integration test could exec a helper binary and
   assert the actual stderr line if stronger evidence is ever wanted.
2. **Malformed `explore.json` yields `Err`** rather than the `grove init` steer. Intentional
   and documented in the function; a corrupt legacy file surfaces a parse error naming the
   path, which is reasonable. No action required.
3. **Minor duplication** — `default_legacy_trace_retain` mirrors explore's private
   `default_trace_retain`; justified because the latter isn't in scope from config.rs.
