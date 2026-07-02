# PROGRESS — GROVE-S02-T04: grove serve explore mode

## Summary

Implemented health-gated explore mode for `grove serve`. When a valid
`.grove/explore.json` config exists and the provider passes a health probe,
the server exposes an exclusive single-tool `explore` surface. When the
provider is unhealthy (unreachable, model missing) or no config exists, the
server transparently falls back to the existing 7-tool standard structural
surface with a diagnostic note on stderr. Mid-session provider loss returns
a recoverable `isError` result.

## Changes Made

### `cli/src/main.rs`
- Replaced unit variant `Serve` with a struct variant carrying:
  - `path: PathBuf` (default `"."`) — project root for locating `.grove/explore.json`
  - `--explore` flag — force explore mode even without a config file
  - `--standard` flag — force standard structural mode (ignore config)
- Updated `Cmd::Serve` dispatch arm: `mcp::serve(&path, explore, standard)?`

### `cli/src/mcp.rs`
- Added imports: `grove_core::explore::{health_probe, run_explore, ExploreConfig, ExploreError, OpenAiCompatClient}` and `std::path::Path`
- Added `Surface` enum (`Standard` | `Explore { cfg, root }`)
- Added `determine_surface(root, force_explore, force_standard) -> Surface`:
  - Precedence: `force_standard` > `force_explore`/config-exists > health check
  - Load failure → `eprintln!` + `Surface::Standard`
  - `health_probe` failure → `eprintln!` + `Surface::Standard`
- Changed `serve()` to `serve(root: &Path, force_explore: bool, force_standard: bool)`:
  - Calls `determine_surface()` once before the request loop
  - Passes `&surface` into `handle()` on every request
- Changed `handle()` to `handle(method, params, surface: &Surface)`:
  - `initialize`: instructions from `explore_instructions(cfg)` vs `instructions()`
  - `tools/list`: single `explore_tool_spec()` vs 7-item `tool_specs()`
  - `tools/call`: `call_explore_tool()` vs `call_tool()`; all other arms unchanged
- Added `explore_instructions(cfg)` — delegation-oriented instructions string
- Added `explore_tool_spec()` — single `explore` tool with plain object schema (no anyOf/oneOf)
- Added `call_explore_tool(params, cfg, root)`:
  - Missing `question` arg → `isError: true` with actionable message
  - `Ok(answer)` → `isError: false` with `answer.text`
  - `ExploreError::ProviderDown` → `isError: true` with endpoint + restart hint
  - `ExploreError::Client` → `isError: true` with message
- Updated all unit tests that call `handle()` to pass `&Surface::Standard`

### `cli/tests/cli.rs`
- Added integration test `explore_mode_unhealthy_provider_falls_back_to_standard_surface`:
  - Creates fixture dir with `.grove/explore.json` pointing to `http://127.0.0.1:1/v1` (port 1 = IANA reserved, guaranteed connection-refused)
  - Spawns `grove serve <dir>`, sends `initialize` + `tools/list` over stdin
  - Asserts `result.tools.len() == 7` (standard fallback surface)
  - Asserts stderr contains `"falling back"`

## Test Evidence

```
running 126 tests
... (all core tests)
test result: ok. 126 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

running 32 tests
... (all cli unit tests including updated handle() calls)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 19 tests
test explore_mode_unhealthy_provider_falls_back_to_standard_surface ... ok
test languages_lists_the_dev_stub ... ok
test lock_writes_grove_lock ... ok
test index_writes_catalog ... ok
test registry_shows_resolved_root ... ok
test init_provisions_and_wires_harness_per_target ... ok
test definition_at_resolves_across_files_python ... ok
test callers_finds_call_sites ... ok
test unknown_extension_errors ... ok
test outline_kind_filter_narrows_results ... ok
test definition_at_resolves_across_files_javascript_aliased ... ok
test definition_at_is_scope_aware ... ok
test check_ok_succeeds_and_broken_exits_nonzero ... ok
test symbols_finds_across_dir ... ok
test outline_lists_definitions_human_and_json ... ok
test source_prints_a_symbol_body ... ok
test definition_by_name_and_by_position ... ok
test map_returns_definitions_with_references ... ok
test invalid_locals_query_is_non_fatal ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

Doc-tests: 1 passed

Total: 178 tests, 0 failed
```

## Files Changed

- `cli/src/main.rs`
- `cli/src/mcp.rs`
- `cli/tests/cli.rs`

## Acceptance Criteria Verification

| AC | Status |
|----|--------|
| AC-1: healthy provider → exclusive `explore` tool surface | ✅ `determine_surface` → `Surface::Explore`; `tools/list` returns 1 tool |
| AC-2: unhealthy provider → transparent fallback (7 tools) | ✅ `health_probe` failure → `Surface::Standard`; covered by integration test |
| AC-3: missing `question` → `isError: true` | ✅ `call_explore_tool` early-return on missing arg |
| AC-4: mid-session provider loss → recoverable `isError` | ✅ `ExploreError::ProviderDown` → `isError: true` with restart hint |
| AC-5: default mode byte-identical | ✅ No config + no flags → `Surface::Standard` → all existing code paths unchanged |
| AC-6: integration test, unhealthy provider fallback | ✅ New test passes |
| Schema: no anyOf/oneOf in `explore_tool_spec` | ✅ Plain `{type: object, properties, required}` |
