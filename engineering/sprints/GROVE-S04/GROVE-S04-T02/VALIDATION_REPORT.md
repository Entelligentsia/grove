# VALIDATION REPORT — GROVE-S04-T02 (standalone review)

**Task:** grove-explore binary — own MCP server identity, explore tool only, startup health gate  
**Verdict:** ✅ Approved

---

## AC1 — `grove-explore` binary, stdio MCP, single `explore` tool

**Pass.**

- `cli/Cargo.toml` contains `[[bin]] name = "grove-explore" path = "src/bin/grove_explore.rs"`.  
- Binary built and confirmed at `target/release/grove-explore`.  
- `SUPPORTED_PROTOCOLS = &["2025-06-18", "2025-03-26", "2024-11-05"]` — byte-identical to `cli/src/mcp.rs`.  
- `tools/list` always returns `[explore_tool_spec()]` — single tool, no branch.  
- Tool name: `"explore"` — identical to mcp.rs.  
- Input schema: `{type:object, properties:{question:{type:string}}, required:["question"]}` — identical.  
- Result contract (`tool_text`): `{content:[{type,text}], isError}` — byte-identical to mcp.rs copy.  
- Note: `explore_tool_spec` description is a trimmed copy of mcp.rs (code review advisory, reconcile in T03). Name, schema, and result contract are the contract elements; description text is informational. AC1 satisfied.  
- Integration test `grove_explore_single_tool_surface_and_identity` asserts `serverInfo.name = "grove-explore"` and `tools.length == 1` and `tools[0].name == "explore"`. **Passed.**

---

## AC2 — Unhealthy provider at startup is a hard failure; server never starts

**Pass.**

- `main()` gate sequence verified in `grove_explore.rs`:  
  1. `GroveConfig::load(&root)` → exit(1) on failure with actionable message.  
  2. `grove_cfg.explore` presence check → exit(1) on `None`.  
  3. `serde_json::from_value::<ExploreConfig>` → exit(1) on deser error.  
  4. `health_probe(&cfg)` → exit(1) on `Err`.  
  5. Only on all-pass does execution reach `serve_explore(...)`.  
- `serve_explore` is only called after the gate; no fallback, no partial startup.  
- Integration test `grove_explore_startup_fails_on_unhealthy_provider`:  
  - Config points at `http://127.0.0.1:1/v1` (port 1 guaranteed connection-refused).  
  - Asserts non-zero exit code ✅  
  - Asserts stderr contains `"is the server running"` or `"base_url"` or `"provider unhealthy"` ✅  
  - Asserts stdout is empty ✅  
  - **Passed.**

---

## AC3 — Mid-session provider loss returns `isError: true`

**Pass.**

- `call_explore_tool` in `grove_explore.rs` pattern matches `ExploreError::ProviderDown { url, detail }` → `Outcome::Ok(tool_text(..., true))`.  
- This is `Outcome::Ok` (not `Outcome::Err`) — the MCP response has `result`, not `error`, with `isError: true` inside — correct per MCP spec for tool-level errors.  
- Message deliberately updated: "restart grove-explore to reconnect" (vs. mcp.rs "restart grove to pick up the structural fallback"). This is correct — grove-explore has no structural fallback. Code review identified this as deliberate, not a defect.  
- `tool_text` implementation is byte-identical to mcp.rs.  
- Covered transitively by `explore/src/agent.rs#provider_down_maps_to_provider_down_error` (54-test suite, all pass).

---

## AC4 — Inner explorer via direct in-process Rust calls

**Pass.**

- `run_explore_reporting(&question, root, cfg, &client, sink, trace)` called directly in `call_explore_tool`.  
- No subprocess spawn, no MCP hop. Confirmed by reading the full function body — the only external call is the library function import.

---

## AC5 — Config read from `.grove/config.json` explore section

**Pass.**

- `GroveConfig::load(&root)` → `grove_cfg.explore` → `serde_json::from_value::<ExploreConfig>(explore_val)`.  
- Identical pattern to the existing explore surface. No config file move. ADR 0002 placement preserved.  
- Integration test 1 writes a full config.json with an explore section and verifies the binary starts successfully (health probe passes).

---

## AC6 — Integration coverage: smoke test + startup-failure test

**Pass.**

- `grove_explore_single_tool_surface_and_identity` (smoke):  
  - Fake `/models` HTTP server on port 0 (OS-assigned).  
  - Sends `initialize` + `tools/list` over piped stdio.  
  - Asserts `serverInfo.name == "grove-explore"`, one tool, tool name `"explore"`.  
  - **Passed.**
- `grove_explore_startup_fails_on_unhealthy_provider` (failure):  
  - Port-1 unreachable config.  
  - Asserts non-zero exit + stderr hint + empty stdout.  
  - **Passed.**
- Both tests passed under `cargo test --release --locked --test cli grove_explore`.

---

## AC7 — Workspace green

**Pass.**

- `cargo test --release --locked`: **337 tests, 0 failed** (135 core + 112 cli unit + 35 cli integration + 54 explore + 1 doc-test).  
- `cargo clippy --all-targets --workspace --locked -- -D warnings`: **clean** (0 warnings).  
- `cli/src/bin/grove_explore.rs` ends with `\n` (0x0a). `cli/Cargo.toml` and `cli/tests/cli.rs` both end with `\n`.

---

## Regression Check

The 33 pre-existing integration tests in `cli/tests/cli.rs` all pass. The `grove` binary is untouched. No existing behaviour changed.

---

## Summary

All 7 acceptance criteria are met. The `grove-explore` binary is self-contained, carries its own MCP identity, exposes exactly one explore tool, enforces a hard startup health gate, calls the inner explorer in-process, and reads config from the existing location. Both mandated integration tests pass. Workspace is green.
