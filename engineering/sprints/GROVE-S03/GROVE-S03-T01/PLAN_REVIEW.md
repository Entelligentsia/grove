# PLAN_REVIEW — GROVE-S03-T01 (standalone review)

## GroveConfig core type + explore section demotion + mode→steering rename

**Verdict:** Approved

---

## Summary

The plan is feasible, complete, and accurate against the current codebase. I
verified the reference model (`core/src/explore/config.rs`) and every call site
of the steering `Mode` enum independently with `grove callers`/`grep`. The
plan's 9-file modification list is exhaustive and correct — no call site is
missing, and no unrelated `mode` field is erroneously swept in.

## Independent Verification

- **Call-site completeness confirmed.** All references to the steering `Mode`
  live in exactly the files the plan lists: `explore/config.rs` (def, `try_from`
  L140, `default` L164, tests), `explore/mod.rs` (re-export L25),
  `explore/agent.rs` (import L25, `cfg.mode` L166/L175, test helper), 
  `explore/steering.rs` (import L16, `system_prompt` sig L50, tests),
  `core/src/lib.rs` (re-export L65), `cli/config_tui/model.rs`,
  `cli/config_tui/update.rs` (struct literal L373), `cli/mcp.rs`
  (`enum_str(&cfg.mode)` L144).
- **Correct exclusions verified.** `app.mode` in `config_tui/view.rs` and
  `update.rs` is the TUI's own `App.mode: usize` index (into `Mode::LEGAL`), NOT
  the steering enum — the plan correctly leaves it untouched. `SessionMeta.mode`
  / `s.mode` in `explore/trace.rs` and `cli/trace_tui/view.rs` are an unrelated
  trace-session string field — correctly excluded.
- **Embedding approach is sound.** `ExploreConfig` already carries a custom
  `Deserialize` impl (config.rs L149) that performs the raw-shape/field-named-
  error parsing internally. `GroveConfig` can therefore hold
  `Option<ExploreConfig>` directly and derive `Deserialize` normally; the inner
  section's error quality is preserved without exposing `RawExploreConfig`.
- **AC → coverage mapping is honest.** All seven ACs trace to concrete plan
  items; the seven-test suite maps to AC6's six required categories plus the
  atomic save/load and missing-file cases.

## Advisory Notes (non-blocking — address during implementation)

1. **Enum variant typo in the plan sketch.** The Data Model section spells the
   `mcp-llm` variant `MpcLlm` (M-p-c). Land it as `McpLlm` to match the domain
   term; only the on-disk spelling (`"mcp-llm"`) is contractual, but a typo'd
   Rust identifier is a lasting readability wart.
2. **`RawGroveConfig` scope can shrink.** Given ExploreConfig's own custom
   `Deserialize`, `RawGroveConfig` need only carry `version: u32` +
   `mode: String` + `explore: Option<ExploreConfig>` for the field-named `mode`
   error. Avoid `Option<serde_json::Value>` for `explore` — it would bypass the
   inner section's validation. The plan lists both options; prefer the former.
3. **`version` validation.** AC1 fixes the wire as `"version": 1`. Consider
   having `validate()` (or `try_from`) reject `version != 1` with a descriptive
   error rather than silently accepting arbitrary integers, consistent with the
   fail-fast idiom the task demands.
4. **Old `"mode"` key in the explore section.** After the rename,
   `RawExploreConfig.steering: String` has no default, so a legacy explore
   object using `"mode"` fails deserialization (missing `steering`). That is the
   correct T01 behaviour (T02 owns migration) — just ensure the
   `steering_key_in_explore_section` test asserts this explicitly rather than
   leaving it ambiguous.
5. **Doc-comment drift.** `cli/config_tui/model.rs` L124 comment references
   `Mode::LEGAL`; update it to `Steering::LEGAL` (or note it stays the TUI's own
   index semantics) so the rename leaves no stale references.

## Testing Strategy Assessment

Adequate. The suite covers serde round-trip per integration mode, explore
present/absent, field-named bad-`mode` error, the `steering` (not `mode`) key,
atomic save→load with no leftover temp file, and the missing-file actionable
error. Existing `ExploreConfig` regression tests are correctly slated for
fixture/assertion updates (`fixed_fixture_deserializes` → `"steering"`,
`provider_serializes_lowercase` → `Steering::Standard`).
