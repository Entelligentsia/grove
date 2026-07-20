# PROGRESS — GROVE-S04-T03

## Summary

Deleted all bimodal Surface/determine_surface/health-probe machinery from
`cli/src/mcp.rs` so that `grove serve` unconditionally dispatches the 7-tool
structural surface. Removed the now-dead `--explore`/`--standard` CLI flags with
a clear replacement-naming error in `cli/src/main.rs`. Replaced the old
`explore_mode_unhealthy_provider_falls_back_to_standard_surface` integration test
with three new tests (AC2, AC3) in `cli/tests/cli.rs`.

## Changes Made

### `cli/src/mcp.rs`
- **Removed imports**: deleted `use grove_core::config::{active_mode, GroveConfig, Mode, ModeChoice}` and the entire `use grove_explore_core::{...}` block (7 types).
- **Deleted `enum Surface`** (`Standard` / `Explore`).
- **Deleted `fn determine_surface`** — the 50-line function that read config, ran a health probe, and returned the surface variant.
- **Simplified `pub fn serve`** — signature changed from `(root: &Path, force_explore: bool, force_standard: bool)` to `(_root: &Path)`; removed `trace_writer`, `surface`, and the `if method == "initialize"` trace-open block.
- **Deleted `fn open_session_trace`** — session trace setup for explore mode.
- **Deleted `fn enum_str`** — helper only used by deleted functions.
- **Simplified `fn handle`** — signature changed from `(method, params, surface, trace)` to `(method, params)`; collapsed the `match surface { Standard => … Explore => … }` branches into direct `instructions()` / `tool_specs()` / `call_tool()` calls.
- **Deleted `fn explore_instructions`** — explore-mode server instructions.
- **Deleted `fn explore_tool_spec`** — the single-tool explore spec JSON.
- **Deleted `const EXPLORE_QUESTION_KEYS` + `fn resolve_explore_question`** — explore argument extraction.
- **Deleted `struct StdoutProgress` + `impl ProgressReporter`** — progress reporting for explore calls.
- **Deleted `fn progress_token`** — MCP progress token extractor.
- **Deleted `fn call_explore_tool`** — explore-mode tool dispatch.
- **Deleted 3 unit tests**: `explore_question_resolves_canonical_and_synonym_keys`, `explore_question_prefers_canonical_over_synonym`, `explore_question_missing_is_none`.
- **Updated 7 `handle()` call sites** in `mod tests` — removed `&Surface::Standard, None` trailing arguments.

### `cli/src/main.rs`
- **Updated `Cmd::Serve` doc comment** to state "Always serves the 7-tool structural surface; use `grove-explore serve` for the explore surface."
- **Hid `--explore` and `--standard` flags** with `#[arg(hide = true)]` — flags still parse (no clap error) so our dispatch guard fires with a useful message.
- **Added dispatch bail! guard**: if `explore || standard`, bail with:
  > "--explore and --standard have been removed from `grove serve`. Use `grove-explore serve` for the LLM-delegating explore surface, or plain `grove serve` for the 7-tool structural surface."
- **Simplified `Cmd::Serve` dispatch** from `mcp::serve(&path, explore, standard)?` to `mcp::serve(&path)?`.

### `cli/tests/cli.rs`
- **Replaced** `explore_mode_unhealthy_provider_falls_back_to_standard_surface` with **`serve_always_serves_structural_surface`** (AC3): same config (mcp-llm + unreachable provider) but now asserts NO "falling back" on stderr — the 7 tools come from the constant surface, not a fallback.
- **Added `serve_removed_explore_flag_errors_with_hint`** (AC2): asserts `grove serve --explore` and `grove serve --standard` exit non-zero with stderr containing "grove-explore serve".
- **Added `serve_mcp_llm_config_returns_7_structural_tools`** (AC3): asserts a project with `mode: "mcp-llm"` in config returns exactly 7 structural tools, with no "explore" tool present.
- **Updated `bug1_serve_mcp_mode_ignores_stale_explore_json`**: updated doc comment to reference T03; changed assertions from "standard surface" language to "constant structural surface" language; removed "falling back" stderr check (invariant now comes from constant surface, not config branching).

## Test Evidence

```
cargo test --release --locked

Running unittests src/main.rs (cli)
running 109 tests
test mcp::tests::both_modes_return_resolved_and_definitions ... ok
test mcp::tests::callers_tool_finds_sites_and_requires_name ... ok
test mcp::tests::check_tool_reports_ok_and_missing_arg ... ok
test mcp::tests::definition_tool_requires_name_or_at ... ok
test mcp::tests::every_tool_schema_is_client_registerable ... ok
test mcp::tests::initialize_echoes_supported_version_else_default ... ok
test mcp::tests::map_tool_returns_definitions_with_references ... ok
test mcp::tests::outline_detail_validates_range ... ok
test mcp::tests::outline_out_of_range_is_tool_error ... ok
test mcp::tests::outline_tool_missing_file_is_error ... ok
test mcp::tests::outline_tool_returns_definitions ... ok
test mcp::tests::ping_and_tools_list_and_notifications ... ok
test mcp::tests::source_and_definition_still_enforce_their_arg_choice ... ok
test mcp::tests::source_tool_by_file_name_and_by_id_and_neither ... ok
test mcp::tests::symbols_tool_lists_definitions ... ok
test mcp::tests::symbols_tool_name_exact_unless_name_contains ... ok
test mcp::tests::tool_text_marks_errors_and_wraps_values ... ok
test mcp::tests::unknown_method_is_method_not_found ... ok
test mcp::tests::unknown_tool_is_invalid_params ... ok
(+90 more passing)
test result: ok. 109 passed; 0 failed; 0 ignored

Running tests/cli.rs
running 37 tests
test serve_always_serves_structural_surface ... ok
test serve_removed_explore_flag_errors_with_hint ... ok
test serve_mcp_llm_config_returns_7_structural_tools ... ok
test bug1_serve_mcp_mode_ignores_stale_explore_json ... ok
(+33 more passing)
test result: ok. 37 passed; 0 failed; 0 ignored

Running unittests src/lib.rs (grove_core)
test result: ok. 135 passed; 0 failed; 0 ignored

Running unittests src/lib.rs (grove_explore_core)
test result: ok. 54 passed; 0 failed; 0 ignored

cargo clippy --all-targets --workspace -- -D warnings → 0 warnings
```

## Acceptance Criteria

| AC | Verified |
|---|---|
| AC1: `Surface`, `determine_surface`, health-probe fallback deleted; zero `explore` imports in mcp.rs | ✅ `grep explore cli/src/mcp.rs` → exit 1 (zero hits) |
| AC2: `serve --explore`/`serve --standard` produce clear error naming `grove-explore serve` | ✅ `serve_removed_explore_flag_errors_with_hint` passes |
| AC3: `grove serve` returns 7 structural tools regardless of `mode: mcp-llm` in config | ✅ `serve_mcp_llm_config_returns_7_structural_tools` + `serve_always_serves_structural_surface` pass |
| AC4: ratatui/crossterm not touched — deferred to T05 | N/A |
| AC5: Existing structural integration tests pass; deleted-flag tests replaced | ✅ all 37 integration tests green |
| AC6: Workspace green — tests, clippy, warning-clean | ✅ 335 tests pass, 0 clippy warnings |

## Files Changed

- `cli/src/mcp.rs` — deleted ~180 lines of explore machinery, simplified serve/handle
- `cli/src/main.rs` — hidden flags, bail! guard, simplified dispatch
- `cli/tests/cli.rs` — replaced 1 test, added 2 new tests, updated 1 test
