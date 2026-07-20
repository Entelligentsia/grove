# CODE REVIEW — GROVE-S04-T07: `doctor` re-keying — explore checks keyed on `grove-explore` registration, not mode

(standalone review)

## Review Summary

Independently verified the working-tree diff (`git diff HEAD`) against the
approved PLAN.md and the five acceptance criteria. The implementation re-keys
the explore check group onto `grove-explore` registration presence, extends
the per-harness drift matrix for the two-registration + stale-layout cases,
removes the orphaned `--explore`/`--standard` doctor flags, and conservatively
prunes `ModeChoice` to a single `None` variant. All four modified files were
read in full diff; the dependent `harness` module was spot-checked to confirm
the assumed primitives (`EXPLORE_SERVER_KEY`, `expected_explore_args`,
`mcp_format`, `mcp_config_path_in` for all six harnesses) exist and behave as
the doctor code assumes.

## Spec Compliance (AC-by-AC)

- **AC1 — explore group iff `grove-explore` registered.** ✓
  `diagnose` now computes `has_explore_registration(root, &home, &harnesses)`
  via a harness-agnostic scan of all six harness configs and gates the
  `explore_checks` block with `if has_explore_reg` (replacing `if mode ==
  Mode::McpLlm`). Verified the helper iterates every selected harness and
  reads the `grove-explore` args via the format-aware `read_server_args`
  (JSON `mcpServers`/`servers` and Codex TOML). Tests
  `mcp_llm_without_explore_registration_skips_explore_group` and
  `mcp_llm_cursor_only_explore_registration_triggers_explore_group` pin both
  sides of the gate, including a non-Claude (Cursor-only) fixture as the plan
  review required.

- **AC2 — structural checks unchanged.** ✓
  The diff to `diagnose` touches only (a) hoisting `harnesses`/`home`/explore
  detection above the harness sub-checks, (b) the explore-group gate line, and
  (c) the `check_harness_serve_surface` argument. Config-load, legal-mode,
  lock-verify, registry/grammars, and version checks are byte-identical. The
  `--json` output mapping and the `if !report.ok() { exit(1) }` exit-code gate
  in `cli/src/main.rs` are untouched.

- **AC3 — drift matrix for two-registration + stale-layout.** ✓
  Both `check_harness_mcp_json` (Claude-dedicated) and `check_harness_registration`
  (per-harness for the other five) now evaluate a structural `grove` entry AND
  an explore `grove-explore` entry against `expected_mcp_args(mode)` /
  `expected_explore_args(mode)`. Stale-layout detection (`has_stale_explore_arg`)
  short-circuits to a `Fail` with the `grove init --as mcp-llm` hint when the
  `grove` entry still carries the removed `--explore` argument. Tests cover:
  stale single-registration under mcp-llm, missing explore entry under
  mcp-llm, unexpected explore entry under mcp (Cursor fixture), and the
  updated `mcp_mode_with_explore_args_in_mcp_json_is_fail`.

- **AC4 — `--explore`/`--standard` removed; `--json` shape + exit code preserved.** ✓
  `Cmd::Doctor` in `cli/src/main.rs` drops the two flag fields and the
  `ModeChoice` force plumbing; the dispatch now passes
  `grove_core::config::ModeChoice::None` directly. `ModeChoice` in
  `core/src/config.rs` is pruned to `None` only, with `active_mode` retained
  for API compatibility (verified all remaining callers — `doctor::diagnose`
  and `init.rs` tests — pass `ModeChoice::None`; no `ForceExplore`/`ForceStandard`
  consumers remain). `doctor_help_documents_verb` strengthened to assert the
  removed flags are absent from `--help`.

- **AC5 — workspace green.** ✓
  `cargo clippy --all-targets --workspace --locked -- -D warnings` finishes
  clean (the single-variant `ModeChoice` match with a named arm does NOT trip
  `clippy::match_single_binding`, as the plan review predicted). All 15 doctor
  unit tests pass, including the five new T07 tests; `doctor_help_documents_verb`
  passes. All four modified files end with a newline (`0a`).

## Code Quality

- **Correctness.** The re-keying is a clean one-line gate change in `diagnose`.
  `has_explore_registration` correctly delegates to `read_server_args` with the
  per-harness `mcp_format()` and `mcp_config_path_in(root, Some(home))`, so
  Codex's global `~/.codex/config.toml` is scanned via the `home` override while
  project-scoped harnesses resolve under `root`. `explore_checks(None)` returns
  a `Fail` with a `grove config` hint, so the `has_explore_reg && cfg_result is
  Err` path degrades gracefully.
- **Security.** Read-only command; no mutation, no network, no user-controlled
  path components (harness paths are `root`/`home` + fixed relative suffixes).
  `serde_json::from_str` and `toml_edit::parse` swallow malformed input to
  `None`. No injection surface.
- **Architecture / layering.** `grove-cst` (core) gains no `grove-explore-core`
  dependency — explore config stays an opaque `serde_json::Value`. The
  conservative `ModeChoice` pruning avoids touching `lib.rs:68` or
  `init.rs:1602/1612`, exactly as the approved plan §4 specifies.
- **Conventions.** Check struct fields, group names (`"explore"`, `"universal"`),
  and the `grove init --as <mode>` hint idiom are preserved. New tests follow the
  existing `tmp`/`write_config`/`write_mcp_json` fixture helpers.

## Advisory Notes (non-blocking)

1. **Stale-layout hint is mode-blind.** `has_stale_explore_arg` returns
   `grove init --as mcp-llm` for *any* declared mode when the `grove` entry
   carries `--explore`. For the primary case (declared `mcp-llm`, pre-split
   layout) this is correct per the plan's drift matrix. For the rare edge case
   of a project downgraded from `mcp-llm` to `mcp` without re-running `init`,
   the hint suggests switching back to `mcp-llm` rather than
   `grove init --as mcp`; the check still `Fail`s correctly, only the suggested
   fix is suboptimal. The plan's drift matrix only specifies the stale-layout
   hint for the `mcp-llm` row, and the implementation matches the approved
   plan, so this is advisory. Worth a future pass to make the hint
   mode-aware (`grove init --as <declared mode>`) if downgrades become common.

2. **Pre-existing duplication retained.** `check_harness_mcp_json` (Claude
   dedicated) and `check_harness_registration` (other harnesses) now both carry
   the full stale/structural/explore comparison matrix. T07 extended both
   consistently with the pre-existing pattern (before T07 they already
   duplicated the structural comparison). A shared helper would reduce the
   surface, but a DRY refactor is out of scope for this task and the
   duplication is defense-in-depth, not a bug.

## Verdict: Approved