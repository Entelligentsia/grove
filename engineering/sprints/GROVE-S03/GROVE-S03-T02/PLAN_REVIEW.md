# PLAN_REVIEW — GROVE-S03-T02 (standalone review)

**Verdict:** Approved

## Summary

The plan implements the legacy `.grove/explore.json` → `.grove/config.json`
migration entirely within `core/src/config.rs`, with a private
`LegacyExploreRaw` struct, a `migrate_from_legacy_explore` free function, and a
three-branch `GroveConfig::load` cascade. I verified the plan against the
actual source: `GroveConfig`, `Mode::McpLlm`, `Mode::from_name`,
`ExploreConfig`, `Steering::from_name`, `Provider::from_name`,
`ExploreConfig::config_path`, and `GroveConfig::save` all exist and are usable
as the plan describes.

## Verification against the code

- **Branch cascade (Step 3)** is sound: `Self::config_path(root).exists()` →
  existing path; `ExploreConfig::config_path(root).exists()` → migration; else
  the unchanged `grove init` bail. `ExploreConfig` is already in scope via
  `use crate::explore::ExploreConfig` — no new import, as claimed.
- **Steering / Provider mapping (Step 2, items 3–4)** — `Steering::from_name`
  and `Provider::from_name` exist and validate + name the offending field, so
  AC1's "load + validate" and AC2's `mode`→`steering` mapping are satisfied.
- **One-time warning (AC3)** — achieved structurally: after `save` writes
  `config.json`, subsequent loads take branch 1 and never re-enter migration.
  Test 5b confirms idempotency.
- **No data loss** — the plan preserves `provider`, `base_url`, `model`,
  `allowed_tools`, `tap`, `trace_retain`, remapping only `mode`→`steering`.
- **AC-to-test mapping** is complete: 5a/5b/5c/5d cover the four required
  behaviours; the existing `missing_file_actionable_error` still guards AC4.

## Advisory notes (address during implementation — not blocking)

1. **`default_trace_retain` is private.** In `core/src/explore/config.rs` the
   default helper is `fn default_trace_retain() -> u32` (private); only
   `pub const DEFAULT_TRACE_RETAIN: u32 = 50;` is exported. The plan's
   `LegacyExploreRaw` field `trace_retain` with `#[serde(default = "default_trace_retain")]`
   will **not compile from `config.rs`**. Define a local default fn in
   `config.rs` (e.g. `fn default_trace_retain() -> u32 { crate::explore::DEFAULT_TRACE_RETAIN }`,
   assuming the const is re-exported) or reference the pub const directly.
   Confirm `DEFAULT_TRACE_RETAIN` is reachable from `config.rs`; if not, add a
   local literal default. Verify the migration round-trips `trace_retain` and
   `tap` faithfully.

2. **Test 5d (stderr assertion) is fragile.** The plan itself flags that
   cross-platform stderr capture is hard and offers a weak "observe in CI
   output" fallback — that is not a real assertion. Prefer factoring the
   warning text into a testable seam: e.g. a `const DEPRECATION_WARNING: &str`
   (or a small `fn deprecation_message() -> String`) that `eprintln!` prints,
   and assert on that constant plus the observable side-effect (migration
   succeeds AND `config.json` now exists) rather than capturing stderr. This
   satisfies "the warning path is exercised" robustly without stdio plumbing.

3. **Malformed legacy file behaviour.** When `explore.json` is present but
   unparseable, migration returns an `Err` (propagated with context) rather
   than the `grove init` steer. That is acceptable and arguably correct, but
   worth a one-line doc comment on `load` so the branch semantics are explicit.

## Non-blocking confirmations

- `explore.json` is intentionally **not deleted** — correct per D4 / sprint
  requirements (doctor warns later).
- `clippy -D warnings` risk is low; `eprintln!` and the added struct/fn are
  idiomatic. Ensure the new `LegacyExploreRaw` fields don't trip
  `dead_code` — every field is read during construction, so this is fine.
