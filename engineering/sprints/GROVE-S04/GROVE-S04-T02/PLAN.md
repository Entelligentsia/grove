# PLAN — GROVE-S04-T02
## grove-explore binary — own MCP server identity, explore tool only, startup health gate

---

## Objective

Create a `grove-explore` binary that runs an MCP server over stdio exposing exactly
one tool (`explore`), with a hard startup gate: an unhealthy provider is a non-zero
exit, never a silent fallback.

---

## Approach

### Binary Placement Decision

The `grove-explore` binary is added as a second `[[bin]]` target inside the
existing `cli/` workspace crate (`cli/src/bin/grove_explore.rs`).

**Rationale:**
- The `cli/` crate already depends on both `grove-cst` and `grove-explore-core`;
  no new workspace member or dependency wiring is needed.
- Integration tests in `cli/tests/cli.rs` can reference the new binary via
  `env!("CARGO_BIN_EXE_grove-explore")` (Cargo sets this env var for every
  `[[bin]]` in the package under test).
- The `grove-explore` binary target inherits all package-level `[dependencies]`
  from `cli/Cargo.toml`.
- Temporary plumbing duplication (~130 lines) from `cli/src/mcp.rs` is acceptable
  because T03 will delete the explore variant from `mcp.rs` shortly after, removing
  the divergence permanently.

`src/bin/grove_explore.rs` is a self-contained compilation unit. Since `cli/` has
no `[lib]` target, it cannot `use` modules from `cli/src/main.rs` or
`cli/src/mcp.rs`. All needed logic lives in the file directly, drawing exclusively
from crate-level dependencies.

### MCP Server Design

The `grove-explore` binary implements a trimmed MCP server that follows
`cli/src/mcp.rs`'s existing pattern:

- **Transport**: stdio, newline-delimited JSON-RPC 2.0 (identical to `grove serve`).
- **Protocol versions**: same `SUPPORTED_PROTOCOLS` (`["2025-06-18", "2025-03-26",
  "2024-11-05"]`) and `DEFAULT_PROTOCOL` (`"2025-06-18"`).
- **Server identity** (`initialize` → `serverInfo`):
  `name = "grove-explore"`, `version = env!("CARGO_PKG_VERSION")`.
- **Tool surface**: exactly one tool — `explore`. `tools/list` always returns
  `[explore_tool_spec()]`. There is no `Surface` enum; the surface is fixed.
- **Tool dispatch**: `tools/call` passes through to `call_explore_tool`, which
  calls `run_explore_reporting` in-process (no MCP hop, no subprocess).
- **Trace/tap support**: if `cfg.tap` is set, a `TraceWriter` is opened on
  `initialize` (same as the current explore path in `grove serve`).
- **Notifications**: `notifications/initialized` and `notifications/cancelled`
  are no-ops (no response). All other unknown methods → JSON-RPC `-32601`.

### Startup Health Gate

The health gate executes before the server enters the stdio loop:

1. **Config load**: `GroveConfig::load(root)` — failure → actionable stderr
   message + `process::exit(1)`.
2. **Explore section**: `grove_cfg.explore` deserialized into `ExploreConfig` —
   missing or invalid → stderr + exit(1).
3. **Health probe**: `health_probe(&cfg)` — `HealthError` display already
   contains fix hints (endpoint, model, config path) — stderr + exit(1) on any
   error.

The server **never starts** (stdin loop is never entered) if any of the above
steps fails. A non-zero exit code is the only outcome.

### Mid-Session Provider Loss (AC3 preservation)

After the server starts, `call_explore_tool` already handles `ExploreError::
ProviderDown` by returning an `isError: true` tool result with a recovery
message (same as the current explore surface). This behavior is preserved
unchanged — no alteration to `run_explore_reporting`.

### Config Source (AC5)

Config is read via `GroveConfig::load(root)` (reads `.grove/config.json`,
with migration from `.grove/explore.json` if needed — identical to what
`determine_surface` does in `cli/src/mcp.rs`). The explore section is
extracted as `grove_cfg.explore: Option<serde_json::Value>` and deserialized
into `ExploreConfig`. No config file move; ADR 0002 placement preserved.

---

## Files to Modify

| Path | Action | Description |
|------|--------|-------------|
| `cli/Cargo.toml` | Modify | Add `[[bin]] name = "grove-explore" path = "src/bin/grove_explore.rs"` |
| `cli/src/bin/grove_explore.rs` | **Create** | Self-contained binary: arg parse, config load, health gate, MCP loop |
| `cli/tests/cli.rs` | Modify | Add two new integration tests (smoke + startup-failure) |

No changes to `Cargo.toml` (workspace root) — `cli/` is already a workspace
member.

---

## grove_explore.rs Module Structure

The binary file contains these internal components (all `fn` / `struct`, not
pub):

| Component | Purpose |
|-----------|---------|
| `struct Args` (clap) | Optional `path` positional arg (default `.`) |
| `fn main()` | Parse args → load config → health probe → enter loop |
| `fn serve_explore(cfg, root)` | Stdio MCP loop: read line, dispatch, write response |
| `fn handle(method, params, cfg, root, trace)` | Dispatcher for all MCP methods |
| `fn explore_tool_spec()` | Returns the JSON tool schema (copied from `mcp.rs`) |
| `fn call_explore_tool(params, cfg, root, trace)` | Dispatches to `run_explore_reporting`; handles `ExploreError` |
| `fn resolve_explore_question(params)` | Extracts `question` / synonym from params |
| `fn tool_text(value, is_error)` | Wraps a value in the MCP content-block shape |
| `fn progress_token(params)` | Extracts `_meta.progressToken` for progress notifications |
| `struct StdoutProgress` | Implements `ProgressReporter` writing JSON-RPC notifications to stdout |
| `fn open_session_trace(cfg, root, params)` | Opens `TraceWriter` on `initialize` when `tap` is set |
| Constants | `SERVER_NAME = "grove-explore"`, `SERVER_VERSION`, `SUPPORTED_PROTOCOLS`, `DEFAULT_PROTOCOL` |

---

## Data Model Changes

None. No new store schema, no config schema change. `ExploreConfig` and
`GroveConfig` are consumed unchanged.

---

## Integration Tests

Both tests are added to `cli/tests/cli.rs`.

### Test 1: `grove_explore_single_tool_surface_and_identity`

**Purpose**: AC1 + AC6 — smoke test verifying server identity and single-tool surface.

**Setup**:
- Bind a `TcpListener` on `127.0.0.1:0` (OS-assigned port) in the test.
- Spawn a thread to accept one connection and respond to the `/models` HTTP
  request with a JSON body listing one model entry matching the configured model
  name (simulating a healthy provider).
- Write `.grove/config.json` with `mode: "mcp-llm"` and an `explore` section
  pointing `base_url` at `http://127.0.0.1:{port}/v1` and the matching model.
- Start `grove-explore <project_dir>` with piped stdio.
- Send `initialize` (id=1) and `tools/list` (id=2) over stdin; close stdin.

**Assertions**:
- `initialize` response: `result.serverInfo.name == "grove-explore"`.
- `tools/list` response: exactly one tool with `name == "explore"`.
- Process exits successfully (health probe succeeded and server ran to EOF).

### Test 2: `grove_explore_startup_fails_on_unhealthy_provider`

**Purpose**: AC2 — hard startup failure when provider is unreachable.

**Setup**:
- Write `.grove/config.json` with `mode: "mcp-llm"` and explore pointing at
  `http://127.0.0.1:1/v1` (IANA reserved, guaranteed connection-refused).
- Start `grove-explore <project_dir>` with piped stdio.
- Do NOT send any MCP messages (the binary must fail before the loop).

**Assertions**:
- Process exits with non-zero status.
- Stderr contains the `HealthError` fix hints (e.g., `"is the server running"` or
  `"base_url"`).
- Stdout is empty (server never entered the loop, never emitted a response).

---

## Testing Strategy

| Coverage | Mechanism |
|----------|-----------|
| Single-tool surface + server identity | Integration test 1 (smoke) |
| Startup-failure on unhealthy provider | Integration test 2 |
| `tools/call` explore dispatch | Existing unit tests in `cli/src/mcp.rs` cover `call_explore_tool`; the new bin replicates the same logic, tested transitively |
| Mid-session provider loss (isError: true) | Existing `ExploreError::ProviderDown` path in `call_explore_tool` — no behaviour change, covered by `explore/src/agent.rs#provider_down_maps_to_provider_down_error` |
| Config loading + health probe | Existing `explore/src/health.rs` + `core/src/config.rs` unit tests |
| Workspace green | `cargo test --release --locked` + `cargo clippy --all-targets --workspace --locked -- -D warnings` |

---

## Acceptance Criteria Mapping

| AC | How satisfied |
|----|--------------|
| AC1 — `grove-explore` binary, stdio MCP, single `explore` tool | New `[[bin]]` in cli/ with the loop from `grove_explore.rs` |
| AC2 — unhealthy provider is startup error, server never starts | Hard-fail health gate before stdin loop in `main()` |
| AC3 — mid-session provider loss returns `isError: true` | `call_explore_tool` ProviderDown → tool-result error path (preserved unchanged) |
| AC4 — inner explorer via direct in-process Rust calls | `run_explore_reporting` called directly, no MCP hop |
| AC5 — config from `.grove/config.json` explore section | `GroveConfig::load` + `grove_cfg.explore` deserialization |
| AC6 — MCP smoke test + startup-failure test | Integration tests 1 and 2 in `cli/tests/cli.rs` |
| AC7 — workspace green | CI gate: `cargo test`, clippy, newline discipline |

---

## Operational Impact

- **New binary**: `grove-explore` ships alongside `grove`. No changes to `grove`
  binary behaviour in this task.
- **No version bump**: rides the next release train per task brief.
- **No registration changes**: those land in T04/T06.
- **No config file changes**: explore section placement unchanged (ADR 0002).
- **No breaking changes** to existing tool schemas or protocol contracts.
- **Materiality**: material (new binary first appears in this task); version
  bump deferred to the release train per sprint decision D3.
