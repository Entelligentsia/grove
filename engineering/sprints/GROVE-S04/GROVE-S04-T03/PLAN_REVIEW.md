# PLAN_REVIEW — GROVE-S04-T03 (standalone review)

**Verdict:** Approved

This is a surgical-deletion plan and it is unusually accurate. I verified every
deletion-safety claim against the actual source rather than trusting the plan.

## Independent Verification Performed

- **Deleted-code inventory matches reality.** Every symbol the plan lists
  (`Surface@32`, `determine_surface@52`, `open_session_trace@168`,
  `enum_str@184`, `explore_instructions@256`, `explore_tool_spec@274`,
  `resolve_explore_question@312`, `StdoutProgress@328`, `progress_token@353`,
  `call_explore_tool@363`, and the 3 explore unit tests @641/653/659) matches
  the `mcp.rs` outline symbol-for-symbol.
- **No dangling references after deletion.** `enum_str` is used only inside
  `open_session_trace` (lines 174–175); `SessionMeta` only at 172; `ExploreError`
  only inside `call_explore_tool` (387, 395); every `grove_explore_core` import
  (`health_probe`, `run_explore_reporting`, `ExploreConfig`, `ExploreError`,
  `OpenAiCompatClient`, `ProgressReporter`, `SessionMeta`, `TraceWriter`) is
  consumed exclusively by deleted functions. Removing the import block is safe.
- **`instructions()` (structural) is correctly retained** and becomes the sole
  instructions path (currently used at line 202).
- **Signature change is contained.** Only `main.rs:405` calls `mcp::serve(...)`;
  the plan updates that call site. No other caller exists.
- **`active_mode`/`Mode`/`ModeChoice` correctly preserved in core.** `Cmd::Doctor`
  (main.rs:291–294) still consumes them, so the plan is right to drop the import
  from `mcp.rs` only and NOT delete the resolver — consistent with the task
  Context note.
- **AC4 dependency claim independently checked.** `ratatui`/`crossterm` are still
  referenced by `cli/src/trace_tui/{mod,view}.rs` and
  `cli/src/config_tui/{mod,view}.rs`. A `cli/Cargo.toml` prune in this task would
  break the build. The plan's deferral to T05 is technically forced and matches
  the task's own "cli/Cargo.toml — dependency prune (with T05)" framing.

## Coverage vs Acceptance Criteria

- AC1 (delete `Surface`/`determine_surface`/fallback; zero explore imports) —
  covered and verified feasible.
- AC2 (removed flags produce a clear replacement-naming error, not silent
  ignore) — covered via hidden `#[arg(hide=true)]` fields + dispatch-arm `bail!`
  and the `serve_removed_explore_flag_errors_with_hint` test. Sound.
- AC3 (`serve` returns 7 structural tools regardless of `mode: mcp-llm`) —
  covered by `serve_mcp_llm_config_returns_7_structural_tools` plus the updated
  `bug1_serve_mcp_mode_ignores_stale_explore_json`.
- AC5 / AC6 (existing tests pass; workspace green) — achievable; the plan
  replaces exactly the two tests (@430 fallback, @523 stale-explore) that
  exercised deleted behavior.

## Advisory Notes (address during implementation — not blockers)

1. **AC4 is a joint T03+T05 gate, not "N/A".** The plan marks AC4 "N/A", which
   undersells it. This task *does* satisfy the serve-path half of AC4 (`mcp.rs`
   carries zero TUI/explore knowledge afterward); the `cargo tree` acceptance
   check only passes once T05 moves the TUI code and prunes the deps. Record
   this coupling in the implementation/commit note so the AC4 checkbox isn't
   silently dropped between T03 and T05.
2. **Update the `Cmd::Serve` doc comment.** `main.rs:182` still reads
   "Project directory used to locate .grove/explore.json" — after this task
   `serve` no longer sniffs `explore.json` at all. Refresh that doc string (and
   the field docs at 185/188 that describe the removed force flags) so the
   `--help`/source doesn't describe deleted behavior.
3. **Confirm clap still parses the hidden flags before the guard fires.** The
   error-with-hint path depends on `--explore`/`--standard` remaining valid
   (hidden) clap args so the dispatch `bail!` is reached rather than clap
   rejecting with an "unexpected argument" message. The test should assert the
   grove-authored hint text, not clap's generic error.
