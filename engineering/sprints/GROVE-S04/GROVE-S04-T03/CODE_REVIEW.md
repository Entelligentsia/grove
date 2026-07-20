# CODE_REVIEW — GROVE-S04-T03 (standalone review)

**Verdict:** Approved

`grove serve` is now a constant 7-tool structural surface. The bimodal
`Surface`/`determine_surface`/health-probe machinery and the `--explore`/`--standard`
flags are gone, and every claim in PROGRESS.md was verified independently against the
working-tree diff, the built binary's test behavior, clippy, and `cargo tree`.

## Independent Verification

Reviewed `git diff HEAD` for the three modified files (changes are uncommitted; task
status `implemented`), not agent reports.

- **AC1 — deletion complete.** `grep -nE "explore|Surface|determine_surface" cli/src/mcp.rs`
  → exit 1 (zero hits). The `grove_explore_core` import block and
  `grove_core::config::{active_mode, GroveConfig, Mode, ModeChoice}` are removed; the
  `grove_core::{ops, registry}` import correctly retained. `enum Surface`,
  `determine_surface`, `open_session_trace`, `enum_str`, `explore_instructions`,
  `explore_tool_spec`, `EXPLORE_QUESTION_KEYS`, `resolve_explore_question`,
  `StdoutProgress`, `progress_token`, `call_explore_tool` all deleted. `handle()` and
  `serve()` signatures simplified; `match surface` branches collapsed to direct
  `instructions()`/`tool_specs()`/`call_tool()`. `instructions()` (structural) retained.
  No dangling refs — clippy `--all-targets --workspace --locked -D warnings` builds clean.
- **AC2 — flag removal with hint.** main.rs hides `--explore`/`--standard`
  (`#[arg(hide = true)]`) so clap still parses them and the dispatch `bail!` fires with a
  message naming `grove-explore serve`. `serve_removed_explore_flag_errors_with_hint`
  passes (both flags exit non-zero, stderr contains `grove-explore serve`).
- **AC3 — constant surface.** `serve_mcp_llm_config_returns_7_structural_tools` and
  `serve_always_serves_structural_surface` both pass: `mode: mcp-llm` in config +
  unreachable provider still yields exactly 7 structural tools, no `explore` tool, and
  (correctly) NO "falling back" on stderr — it is the constant surface, not a fallback.
- **AC5 — tests updated, not weakened.** `bug1_serve_mcp_mode_ignores_stale_explore_json`
  keeps the stale-`explore.json` regression guard, retargeted to the constant-surface
  invariant. The replaced test's assertion was inverted (`contains` → `!contains`
  "falling back") — the correct semantic flip.
- **AC6 — workspace green.** `cargo clippy --all-targets --workspace --locked -D warnings`
  clean; `cargo test --release --locked -p grove-cst-cli` → 37 integration tests pass;
  `--bins` → 109 unit tests pass (all 7 updated `handle()` call sites compile and pass).

## AC4 — accepted cross-task deferral (not a blocker)

AC4 (ratatui/crossterm no longer deps of the `grove` binary) is **not** satisfied by this
task: `cargo tree -i ratatui` still shows `grove-cst-cli` as the consumer, because
`trace_tui/` and `config_tui/` still pull them. This is the sanctioned deferral recorded
in the approved PLAN and PLAN_REVIEW ("a Cargo.toml prune here would break the build;
deferral to T05 is forced"). T03 covers the serve-path decoupling only, and is consistent
with T05's planned TUI move.

**Follow-up dependency (advisory, tracked):** the AC4 `cargo tree` check is a joint
T03+T05 gate — it must be run and pass when T05 lands the TUI move + Cargo.toml prune.
Do not let it fall through the cracks.

## Advisory Notes

1. **Cosmetic blank-line runs** left by the deletions: `cli/src/mcp.rs:23` (one extra
   blank before `serve`) and `cli/src/mcp.rs:126` (three consecutive blanks before the
   tool catalogue). Not a gate — CI deliberately omits `cargo fmt --check` (ci.yml:26-27,
   the tree is intentionally denser than rustfmt default) — but collapsing them to a
   single blank would tidy the diff. Non-blocking.
