# VALIDATION REPORT — GROVE-S02-T04 (standalone review)

**Task:** `grove serve` explore mode — exclusive surface (healthy) / structural fallback (unhealthy)
**Validator:** grove QA Engineer 🍵
**Date:** 2026-07-02

---

**Verdict:** Approved

---

## Test Suite Results

Full `cargo test --release --locked` run against the committed implementation:

```
grove_core:   126 passed; 0 failed
grove-cli:     32 passed; 0 failed
cli (integration): 19 passed; 0 failed
doc-tests:      1 passed; 0 failed
Total: 178 passed; 0 failed
```

`cargo clippy --release --locked --all-targets -- -D warnings` → **clean** (0 warnings, 0 errors).

---

## Acceptance Criteria Evaluation

### AC-1 — Mode selection: config/flag routing, force precedence, mcp.rs stays thin ✅ PASS

**Evidence:**
- `main.rs`: `Serve` variant is a struct with `path: PathBuf` (default `.`), `--explore` bool, `--standard` bool (clap flags). `Cmd::Serve` arm dispatches to `mcp::serve(&path, explore, standard)`.
- `mcp.rs`: `determine_surface(root, force_explore, force_standard)` implements the mandated precedence:
  1. `force_standard` → return `Surface::Standard` immediately
  2. `force_explore || ExploreConfig::config_path(root).exists()` → attempt explore
  3. `ExploreConfig::load()` failure → fallback with stderr note
  4. `health_probe(&cfg)` failure → fallback with stderr note
  5. All good → `Surface::Explore { cfg, root }`
- `core::explore` executes; `mcp.rs` only branches on results.

### AC-2 — Startup gate decides surface; healthy → 1 tool (plain schema); unhealthy → 7-tool fallback ✅ PASS

**Evidence:**
- `health_probe(&cfg)` called once in `determine_surface()` before the request loop.
- `Surface::Explore` path: `tools/list` → `json!({ "tools": [explore_tool_spec()] })` — exactly 1 tool.
- `explore_tool_spec()` schema: `{ "type": "object", "properties": { "question": { "type": "string", ... } }, "required": ["question"] }` — plain object, no top-level `anyOf`/`oneOf` (code-verified; grep of `anyOf|oneOf` shows only comments and test assertions, not schema values).
- `initialize` response in explore mode returns `explore_instructions(cfg)` — delegation-oriented, names model and base_url.
- `health_probe` failure → `Surface::Standard` + `eprintln!("grove serve: explore provider unhealthy ({e}); falling back to standard structural surface")`.
- Integration test `explore_mode_unhealthy_provider_falls_back_to_standard_surface` verifies: port-1 URL → 7 tools in `tools/list` + "falling back" in stderr + `initialize` succeeds.

### AC-3 — tools/call on explore runs run_explore; all error paths return isError:true ✅ PASS

**Evidence:**
- `call_explore_tool()` in `mcp.rs`:
  - Missing `question` argument → `Outcome::Ok(tool_text(..., true))` — `isError: true`.
  - `Ok(answer)` → `Outcome::Ok(tool_text(&json!(answer.text), false))`.
  - `ExploreError::ProviderDown` → `Outcome::Ok(tool_text(..., true))` — `isError: true`.
  - `ExploreError::Client(msg)` → `Outcome::Ok(tool_text(&json!(msg), true))` — `isError: true`.
  - Match is exhaustive over all `ExploreError` variants.
- Uses real `OpenAiCompatClient::new(cfg)` and `run_explore()`.
- No JSON-RPC error responses for tool failures (all return `Outcome::Ok` with `isError`).
- *Advisory (not blocking):* No direct unit test for `call_explore_tool` missing-question path; exercised only transitively through the integration path. Code path correct.

### AC-4 — Mid-session provider loss → recoverable isError, no crash ✅ PASS

**Evidence:**
- `ExploreError::ProviderDown { url, detail }` arm returns:
  ```
  "provider down ({url}): {detail}; check the endpoint / run `grove config` / restart grove to pick up the structural fallback"
  ```
  as `Outcome::Ok(tool_text(..., true))` — `isError: true`, actionable message present.
- The server loop continues after this return; no `process::exit` or panic. The `serve()` loop's `Outcome::Ok(result)` branch writes the response and continues.

### AC-5 — Default mode byte-identical to existing behaviour ✅ PASS

**Evidence:**
- No `.grove/explore.json` + no `--explore` flag → `determine_surface()` returns `Surface::Standard`.
- Standard branch: `tools/list` → `tool_specs()` (7 tools unchanged), `initialize` instructions → `instructions()` (unchanged), `tools/call` → `call_tool()` (unchanged).
- All 32 existing `mcp::tests` unit tests pass with `&Surface::Standard` argument added — assertions unchanged.
- All 19 integration tests pass, including all existing MCP smoke tests.

### AC-6 — Integration test: unhealthy provider → 7-tool fallback surface + stderr note ✅ PASS

**Evidence:**
- `cli/tests/cli.rs`: `explore_mode_unhealthy_provider_falls_back_to_standard_surface` test:
  - Creates `.grove/explore.json` with `base_url: "http://127.0.0.1:1/v1"` (port-1 = connection refused).
  - Spawns `grove serve <dir>`.
  - Sends `initialize` (id=1) and `tools/list` (id=2).
  - Asserts `tools.len() == 7` (fallback surface).
  - Asserts `stderr.contains("falling back")`.
- Test passes: `test explore_mode_unhealthy_provider_falls_back_to_standard_surface ... ok`.

### AC-7 — Warning-clean, clippy clean, tests green; no "fastcontext" string ✅ PASS

**Evidence:**
- `cargo test --release --locked` → 178 pass, 0 fail.
- `cargo clippy --release --locked --all-targets -- -D warnings` → `Finished` with no diagnostics.
- `grep -rn "fastcontext" cli/src/ core/src/` → **CLEAN** (no matches in source files).

---

## Advisory Notes (not blocking)

1. **Missing-question isError** and **`explore_tool_spec()` no-anyOf/oneOf shape** lack direct unit guards (plan review advisories, carried into code review). Both are exercised transitively; code is correct.  
2. **`explore_instructions` model/base_url interpolation** has no unit test. Low risk; formatting verified by code read.

---

## Summary

All seven acceptance criteria pass. The implementation is deterministically correct under test (178 green), warning-free, clippy-clean, and contains no prohibited "fastcontext" naming in source files. The health-gated surface selection, explore tool spec schema, fallback behaviour, mid-session recovery, and byte-identical default mode are all verified by code inspection plus test evidence.
