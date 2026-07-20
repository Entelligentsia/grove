# CODE_REVIEW — GROVE-S04-T02 (standalone review)

## grove-explore binary — own MCP server identity, explore tool only, startup health gate

**Verdict:** Approved

Independently verified against the approved PLAN.md and the task prompt's AC1–AC7.
I read the actual source (`cli/src/bin/grove_explore.rs`, `cli/Cargo.toml`,
`cli/tests/cli.rs`), diffed the reused logic against the reference in
`cli/src/mcp.rs`, and re-ran the build, the two new integration tests, and
clippy myself rather than trusting PROGRESS.md.

### Independent verification
- `cargo build --release --locked --bin grove-explore` — clean.
- `cargo test --release --locked --test cli grove_explore` — **2 passed**
  (`grove_explore_single_tool_surface_and_identity`,
  `grove_explore_startup_fails_on_unhealthy_provider`).
- `cargo clippy --all-targets --workspace --locked -- -D warnings` — clean.
- `cli/src/bin/grove_explore.rs` ends with a trailing newline.

### AC compliance
| AC | Status | Evidence |
| --- | --- | --- |
| AC1 — grove-explore bin, stdio JSON-RPC 2.0, single `explore` tool, same protocols | ✅ | `[[bin]]` in `cli/Cargo.toml`; `SUPPORTED_PROTOCOLS` identical to `grove serve`; `tools/list` returns exactly `[explore_tool_spec()]`; smoke test asserts single tool named `explore`. Tool **name**, **input schema** (`question` required), and **result contract** (`tool_text`) match today's surface. |
| AC2 — unhealthy provider = startup error, never a different surface | ✅ | `main()` gate: `GroveConfig::load` → explore deser → `health_probe`, each `process::exit(1)`, all *before* `serve_explore`. Startup-fail test asserts non-zero exit, stderr fix hint, **empty stdout** (loop never entered). |
| AC3 — mid-session provider loss → `isError: true` | ✅ | `call_explore_tool` ProviderDown arm returns `tool_text(..., true)` — behavior preserved from `mcp.rs` (message text correctly re-pointed, see Advisory 1). |
| AC4 — inner explorer via direct in-process Rust calls | ✅ | `run_explore_reporting(&question, root, cfg, &client, sink, trace)` called directly — no MCP hop, no subprocess. |
| AC5 — config from `.grove/config.json` explore section, no file move | ✅ | `GroveConfig::load(&root)` then `grove_cfg.explore` → `ExploreConfig`; no config-file relocation. |
| AC6 — MCP smoke test + startup-failure test | ✅ | Both present in `cli/tests/cli.rs`, both pass under my own run. |
| AC7 — workspace green | ✅ | build + tests + clippy `-D warnings` all clean; newline OK. |

### Correctness / architecture notes
- Startup gate ordering and hard-fail semantics match the plan exactly; no
  fallback path exists — a second binary cleanly expresses the always-explore mode.
- Single-threaded synchronous loop: `StdoutProgress::report` re-locks the global
  stdout and writes whole lines during `run_explore_reporting` (which completes
  before the response line is written), so there is no interleaving/corruption
  risk with the response writer. ✓
- Trace/tap parity preserved: `TraceWriter` opened on `initialize` when `cfg.tap`.

### Advisory notes (non-blocking)
1. **ProviderDown message deliberately re-pointed** — `mcp.rs` says "restart grove
   to pick up the structural fallback"; the binary says "restart grove-explore to
   reconnect". This is the *correct* adaptation: grove-explore has no structural
   fallback. AC3's contract (recoverable `isError:true` with an actionable hint)
   is preserved. Good judgment, not a defect.
2. **`explore_tool_spec` description is trimmed vs `mcp.rs`** — the binary omits the
   trailing "Do NOT hand it one compound…/Best flow…" subagent-orchestration
   guidance. The plan's Files table said "copied from mcp.rs"; it is actually a
   shortened copy. AC1's enumerated invariants (name, input schema, result
   contract) are all satisfied, so this is compliant — but if the two surfaces are
   meant to present an identical tool description to callers, reconcile them in T03
   when the shared spec is de-duplicated. Worth a deliberate decision, not a silent drift.
3. **Config Step-2 error string hard-codes "mode is mcp-llm"** — it prints "mode is
   mcp-llm but no explore section found" without verifying the mode. If a user
   points grove-explore at a non-mcp-llm config, the diagnostic is slightly
   inaccurate. Low impact (diagnostic text only); consider softening in T03.

### T03 hand-off reminder
~130 lines are intentionally duplicated from `mcp.rs` (tool spec, `call_explore_tool`,
`tool_text`, `StdoutProgress`, `resolve_explore_question`). This is by design and is
cleared by T03 — keep the T02→T03 ordering tight so the duplication window stays short.
