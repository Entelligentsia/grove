# PLAN_REVIEW — GROVE-S04-T02 (standalone review)

**Verdict:** Approved

## Summary

The plan for the `grove-explore` binary is well-grounded, feasible, and maps
cleanly to all seven acceptance criteria. I verified every load-bearing
assumption against the actual code rather than the plan's prose. The core design
insight — that the new binary is the current explore surface with its *fallback*
turned into a *hard exit* — is correct and precisely the AC2 requirement.

## Independent verification performed

1. **Public API surface (all confirmed exported from `grove_explore_core`)** —
   `explore/src/lib.rs` re-exports `run_explore_reporting`, `ExploreError`,
   `ProgressReporter`, `NoopReporter`, `OpenAiCompatClient`, `health_probe`,
   `HealthError`, `ExploreConfig`, `TraceWriter`, `SessionMeta`. `GroveConfig`
   is in `grove_core`. The self-contained bin can `use` all of these from
   crate-level deps — no missing wiring.
2. **Fallback→exit distinction (AC2)** — `cli/src/mcp.rs#determine_surface`
   currently returns `Surface::Standard` on config-load, explore-deser, and
   `health_probe` failures. The plan correctly replaces each of these three
   fallback arms with `process::exit(1)`. This is the crux of the task and the
   plan gets it right.
3. **AC3 preservation** — `call_explore_tool` maps `ExploreError::ProviderDown`
   to an `isError:true` tool result; replicating this verbatim preserves the
   mid-session recovery behavior. No change to `run_explore_reporting`.
4. **Config shape (AC5)** — `core/src/config.rs` defines
   `GroveConfig { mode: Mode (kebab `mcp-llm`), explore: Option<Value> }`. The
   direct `config.json` write in Test 1 matches an existing fixture at
   config.rs:607, so the test setup is feasible.
5. **Test feasibility (AC6)** — Test 2 mirrors the existing
   `explore_mode_unhealthy_provider_falls_back_to_standard_surface` test (port-1
   connection-refused). `HealthError`'s `Display` (health.rs:48) emits
   "is the server running? check `base_url` …", so the stderr fix-hint assertion
   is grounded. `env!("CARGO_BIN_EXE_grove-explore")` resolves for a second
   `[[bin]]` (existing tests use `CARGO_BIN_EXE_grove`).
6. **Placement** — `cli/` has exactly one `[[bin]]` and no `[lib]`; adding a
   second `[[bin]]` inheriting package deps is valid Cargo. The self-contained
   `src/bin/grove_explore.rs` is the correct consequence of the no-lib constraint.

## Advisory notes (non-blocking)

1. **Temporary duplication risk.** The plan copies ~130 lines of MCP plumbing
   from `mcp.rs` and relies on T03 to delete the divergent original. If T03
   slips, two hand-maintained MCP loops coexist. This is acknowledged and
   accepted in the plan; just keep the T02→T03 ordering tight so the window
   stays short.
2. **Mode gate intentionally skipped.** Unlike `determine_surface`, the new
   binary does not consult `active_mode`/`Mode::McpLlm` — it *is* the explore
   surface, so it always serves explore. Config *source* (the `explore` section)
   is identical, satisfying AC5. Worth a one-line comment in `main()` stating
   the mode gate is deliberately absent, so a future reader doesn't mistake it
   for an omission.
3. **`HealthError` wording references `.grove/explore.json`** (pre-existing),
   not `config.json`. Test 2 should assert on the stable substrings
   (`base_url` / "is the server running") rather than the file name, as the plan
   already proposes. No action needed for this task.
4. **Test 1 timing.** The fake `/models` server must return a body whose model
   name matches the configured `model` for `health_probe` to pass (health.rs
   checks the model is listed). The plan notes this; ensure the accept-thread
   serves before the probe's `CONNECT_TIMEOUT` elapses (a single blocking
   `accept()` on a bound listener is sufficient).

None of the above blocks implementation. Proceed.
