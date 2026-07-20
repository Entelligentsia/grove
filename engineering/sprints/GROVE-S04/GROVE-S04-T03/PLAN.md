# PLAN — GROVE-S04-T03: `grove serve` always structural
## Delete `Surface`/`determine_surface`/serve mode flags/health fallback

**Status:** planned  
**Depends on:** GROVE-S04-T01 (explore crate split)  
**Runs in parallel with:** GROVE-S04-T02 (grove-explore binary)

---

## Objective

Make `grove serve`'s surface a constant. Delete the bimodal machinery —
`Surface`, `determine_surface`, the `serve --explore`/`--standard` flags, and
the silent health-probe fallback — so `grove serve` unconditionally serves the
7-tool structural surface. `cli/src/mcp.rs` carries zero explore knowledge
after this task.

---

## Approach

This is a surgical deletion task. The approach is:

1. **Simplify `cli/src/mcp.rs`** — strip every explore-specific code path; the
   file becomes a pure structural MCP dispatcher.
2. **Update `cli/src/main.rs`** — remove `--explore`/`--standard` from the
   `Serve` subcommand; add a hidden-flag guard that returns a clear,
   actionable error for callers who pass the now-dead flags.
3. **Update `cli/tests/cli.rs`** — replace the old fallback test with a new
   constant-surface test; rename the stale-explore test to reflect the new
   always-structural invariant; add a flag-removed error test and an
   integration test for `mode: mcp-llm` + constant structural response.

No logic is moved to another module — it is deleted. `grove_explore_core` is
no longer imported in `cli/src/mcp.rs` (satisfying AC1). The `active_mode` /
`GroveConfig` / `Mode` / `ModeChoice` imports are also dropped because their
only consumer (`determine_surface`) is gone.

### Constraint: no Cargo.toml change in this task

`cli/Cargo.toml` keeps `grove-explore-core`, `ratatui`, and `crossterm` as
dependencies because `cli/src/init.rs`, `cli/src/tap.rs`, and
`cli/src/config_tui/` still reference them. The dependency prune is T05's
scope (Artifacts Involved: "cli/Cargo.toml — dependency prune (with T05)").

---

## Files to Modify

| File | Change |
|------|--------|
| `cli/src/mcp.rs` | Delete `Surface` enum, `determine_surface`, `open_session_trace`, `explore_instructions`, `explore_tool_spec`, `resolve_explore_question`, `StdoutProgress`, `progress_token`, `call_explore_tool`, `enum_str`; remove all `grove_explore_core` imports; remove `active_mode`/`GroveConfig`/`Mode`/`ModeChoice` from `grove_core::config` import; simplify `serve()` signature (drop `force_explore`, `force_standard`); simplify `handle()` signature (drop `surface`, `trace` params); remove `trace_writer` from the serve loop; drop explore-specific unit tests |
| `cli/src/main.rs` | Remove `explore: bool` and `standard: bool` fields from `Cmd::Serve`; keep them as `#[arg(hide = true)]` hidden fields; add a guard in the `Cmd::Serve` dispatch arm that bails with a clear error naming `grove-explore serve` / plain `grove serve`; update the `mcp::serve` call to pass only `&path` |
| `cli/tests/cli.rs` | Replace `explore_mode_unhealthy_provider_falls_back_to_standard_surface` with `serve_always_serves_structural_surface` (AC3); update `bug1_serve_mcp_mode_ignores_stale_explore_json` comment/assertions to reflect always-structural invariant; add `serve_removed_explore_flag_errors_with_hint` (AC2); add `serve_mcp_llm_mode_still_returns_7_structural_tools` (AC3) |

---

## Data Model Changes

None. No schema changes to `.forge/store/` or `.forge/config.json`. No new
types introduced.

---

## Deleted Code Inventory

### `cli/src/mcp.rs`

**Deleted items:**

| Symbol | Lines (approx) | Reason |
|--------|---------------|--------|
| `enum Surface` | 32–37 | Bimodal dispatch enum; always Standard now |
| `fn determine_surface` | 52–101 | Reads config + health-probes; no longer needed |
| `fn open_session_trace` | 168–182 | Explore-only session tracing |
| `fn enum_str` | 184–189 | Only used by `open_session_trace` and `explore_instructions` |
| `fn explore_instructions` | 256–273 | Explore-only server instructions string |
| `fn explore_tool_spec` | 274–311 | Explore tool JSON spec |
| `fn resolve_explore_question` | 312–327 | Explore argument extraction |
| `struct StdoutProgress` | 328–332 | Explore progress reporter |
| `impl ProgressReporter for StdoutProgress` | 333–352 | " |
| `fn progress_token` | 353–362 | " |
| `fn call_explore_tool` | 363–409 | Explore tool dispatch |
| 3 `resolve_explore_question` unit tests | 641–668 | Tests for deleted function |

**Deleted imports:**

```rust
// From grove_core::config — all used only by determine_surface:
use grove_core::config::{active_mode, GroveConfig, Mode, ModeChoice};

// Entire grove_explore_core import block:
use grove_explore_core::{
    health_probe, run_explore_reporting, ExploreConfig, ExploreError, OpenAiCompatClient,
    ProgressReporter, SessionMeta, TraceWriter,
};
```

**Signature changes:**

```rust
// Before:
pub fn serve(root: &Path, force_explore: bool, force_standard: bool) -> Result<()>
fn handle(method: &str, params: &Value, surface: &Surface, trace: Option<&TraceWriter>) -> Outcome

// After:
pub fn serve(root: &Path) -> Result<()>
fn handle(method: &str, params: &Value) -> Outcome
```

**`serve()` loop simplification:** remove `let surface = determine_surface(...)`,
`let mut trace_writer`, the `if method == "initialize"` trace-open block, and
`trace_writer.as_ref()` from the `handle(...)` call.

**`handle()` simplification:** remove the `Surface`-matching branches in
`"initialize"`, `"tools/list"`, `"tools/call"`. Each arm becomes a single
constant path:
- `"initialize"` → always `instructions()` (structural)
- `"tools/list"` → always `tool_specs()` (7 structural tools)
- `"tools/call"` → always `call_tool(params)` (structural dispatch)

**Updated unit test signatures** (inside `mod tests`): the `call` helper and
all `handle(...)` call sites drop the `&Surface::Standard, None` trailing args.

### `cli/src/main.rs`

**`Cmd::Serve` before:**

```rust
Serve {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long = "explore")]
    explore: bool,
    #[arg(long = "standard")]
    standard: bool,
},
```

**`Cmd::Serve` after:**

```rust
Serve {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Removed in ADR 0004 Stage 2 — use `grove-explore serve` instead.
    #[arg(long = "explore", hide = true)]
    explore: bool,
    /// Removed in ADR 0004 Stage 2 — plain `grove serve` is always structural.
    #[arg(long = "standard", hide = true)]
    standard: bool,
},
```

**Dispatch arm after:**

```rust
Cmd::Serve { path, explore, standard } => {
    if explore || standard {
        anyhow::bail!(
            "`grove serve --explore` and `grove serve --standard` have been removed \
             (ADR 0004 Stage 2).\n\
             · For LLM explore-mode access, use: grove-explore serve\n\
             · For the 7-tool structural surface (the default), use: grove serve"
        );
    }
    mcp::serve(&path)?;
}
```

---

## Testing Strategy

### Unit tests (in `cli/src/mcp.rs`)

1. **Delete** the three `resolve_explore_question` tests (they test a deleted
   function).
2. **Update** the `call` helper to remove the `&Surface::Standard, None`
   arguments.
3. **Update** `initialize_echoes_supported_version_else_default`,
   `ping_and_tools_list_and_notifications`, and `unknown_method_is_method_not_found`
   to call `handle(method, params)` without surface/trace args.
4. All remaining unit tests (`outline_detail_validates_range`,
   `both_modes_return_resolved_and_definitions`, every tool test) are
   unchanged in logic; only the `handle` call sites within the `call` helper
   are updated.

### Integration tests (in `cli/tests/cli.rs`)

| Test name | Action | Asserts |
|-----------|--------|---------|
| `serve_always_serves_structural_surface` | New (replaces `explore_mode_unhealthy_provider_falls_back_to_standard_surface`) | `grove serve` with `mode: mcp-llm` in config.json + unreachable provider returns exactly 7 structural tools; no "falling back" assertion (it's not a fallback — it's the constant surface) |
| `bug1_serve_mcp_mode_ignores_stale_explore_json` | Update comment + remove "falling back" stderr assertion | 7 tools, no "explore" tool — invariant now comes from constant surface, not config branching |
| `serve_removed_explore_flag_errors_with_hint` | New | `grove serve --explore` exits non-zero; stderr contains "grove-explore serve" |
| `serve_mcp_llm_config_returns_7_structural_tools` | New (AC3) | A directory with `.grove/config.json` containing `mode: "mcp-llm"` returns 7 structural tools from `tools/list` |

### Build / lint verification

```
cargo test --release --locked
cargo clippy --all-targets --workspace --locked -- -D warnings
```

Both must be green and warning-clean.

---

## Acceptance Criteria Checklist

| AC | Description | Verified by |
|----|-------------|-------------|
| AC1 | `Surface`, `determine_surface`, health-probe fallback deleted from `cli/src/mcp.rs`; zero `explore` imports | `grep -n explore cli/src/mcp.rs` returns 0 hits; `cargo build` clean |
| AC2 | `serve --explore` / `serve --standard` produce clear error naming the replacement | `serve_removed_explore_flag_errors_with_hint` integration test |
| AC3 | `grove serve` returns 7 structural tools regardless of `mode: mcp-llm` in config | `serve_mcp_llm_config_returns_7_structural_tools` integration test |
| AC4 | ratatui/crossterm are NOT decoupled in this task — deferred to T05 | N/A (noted in Cargo.toml constraint above) |
| AC5 | Existing structural integration tests pass; deleted-flag tests are replaced | `cargo test --release --locked` green |
| AC6 | Workspace green: tests, clippy, warning-clean | CI commands above |

---

## Operational Impact

- **Material change:** Yes. This removes publicly visible CLI flags (`--explore`,
  `--standard`) from `grove serve`. Existing installations that invoke
  `grove serve --explore` (e.g. via a manually edited `.mcp.json`) will start
  getting a clear error message. The CHANGELOG should note this deprecation.
- **Sequencing:** ships with T04 (init migration). The intermediate state
  (T03 committed, T04 not yet) is never released to end-users.
- **`active_mode` resolver:** loses its `serve` consumer here but remains in
  `grove_core::config` because `init` and `doctor` still use it. Do NOT delete
  it in this task.
- **No ratatui/crossterm change** in this task — those stay until T05.
