# PLAN — GROVE-S02-T04: grove serve explore mode

## Objective

Add a second surface to `grove serve`: in **explore mode** (gated by `.grove/explore.json` presence or an explicit CLI flag), the server exposes **exactly one** `explore` tool to the outer agent when the provider is healthy, and **falls back to the unchanged 7-tool structural surface** when the provider is unhealthy at startup. Mid-session provider loss returns a recoverable `isError: true` result rather than crashing.

---

## Approach

The surface is determined **once at startup** before the request loop begins (Decision D3). A lightweight `Surface` enum carries the runtime state; all per-request dispatch branches on it. The unhealthy-startup path re-uses the existing code paths verbatim — no re-implementation.

### Component breakdown

| Area | What changes |
|---|---|
| `cli/src/main.rs` | `Serve` variant gains `path: PathBuf` (project root) + `--explore` / `--standard` override flags |
| `cli/src/mcp.rs` | New `Surface` enum; `serve()` and `handle()` become surface-aware; three new explore-mode helpers added |
| `cli/tests/cli.rs` | One new deterministic integration test (AC-6) |

---

## Files to Modify

### `cli/src/main.rs`

**`Serve` variant** — currently `Serve` (unit variant). Replace with a struct variant:

```
Serve {
    /// Project directory used to locate .grove/explore.json (default: current dir).
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Force explore mode even if .grove/explore.json is absent.
    #[arg(long = "explore")]
    explore: bool,
    /// Force standard structural mode (ignore .grove/explore.json if present).
    #[arg(long = "standard")]
    standard: bool,
}
```

**`main()` dispatch** — update the arm:

```
Cmd::Serve { path, explore, standard } => mcp::serve(&path, explore, standard)?,
```

### `cli/src/mcp.rs`

**New imports** (in addition to existing `grove_core::{ops, registry}`):

```rust
use grove_core::explore::{
    health_probe, run_explore, ExploreConfig, ExploreError, OpenAiCompatClient,
};
use std::path::{Path, PathBuf};
```

**New `Surface` enum** (private, local to `mcp.rs`):

```rust
enum Surface {
    Standard,
    Explore { cfg: ExploreConfig, root: PathBuf },
}
```

**`determine_surface(root, force_explore, force_standard) -> Surface`** — private:
1. If `force_standard` → return `Surface::Standard` immediately.
2. Compute `is_explore = force_explore || ExploreConfig::config_path(root).exists()`.
3. If not `is_explore` → `Surface::Standard`.
4. Load config via `ExploreConfig::load(root)`. On load failure → `eprintln!` and `Surface::Standard`.
5. Call `health_probe(&cfg)`:
   - `Ok(())` → `Surface::Explore { cfg, root: root.to_path_buf() }`.
   - `Err(e)` → `eprintln!("grove serve: explore provider unhealthy ({e}); falling back to standard structural surface")` and `Surface::Standard`.

**`pub fn serve(root: &Path, force_explore: bool, force_standard: bool) -> Result<()>`** — replaces `pub fn serve() -> Result<()>`:
- Calls `determine_surface(root, force_explore, force_standard)` before the request loop.
- Passes `&surface` into `handle()` on every request.

**`fn handle(method: &str, params: &Value, surface: &Surface) -> Outcome`** — replaces current `handle(method, params)`:
- `initialize`: same protocol negotiation; `instructions` field comes from `match surface { Standard => instructions(), Explore { cfg, .. } => explore_instructions(cfg) }`.
- `tools/list`: `Standard` → `json!({ "tools": tool_specs() })`; `Explore { .. }` → `json!({ "tools": [explore_tool_spec()] })`.
- `tools/call`: `Standard` → `call_tool(params)` (unchanged); `Explore { cfg, root }` → `call_explore_tool(params, cfg, root)`.
- All other arms (`ping`, notifications, unknown method) unchanged.

**`fn explore_instructions(cfg: &ExploreConfig) -> String`** — new private helper:
- Returns a string explaining that grove is in explore mode delegating to `cfg.model` at `cfg.base_url`, and describing the `explore` tool. Explicitly notes that provider loss returns an actionable `isError` and the user can restart to get the structural fallback.

**`fn explore_tool_spec() -> Value`** — new private helper:
- Returns a single-item JSON object for the `explore` tool:
  - `name`: `"explore"`
  - `description`: brief description of delegation behaviour
  - `inputSchema`: `{ "type": "object", "properties": { "question": { "type": "string", "description": "..." } }, "required": ["question"] }` — no `anyOf`/`oneOf` at the top level (MCP checklist)
  - `annotations`: `{ "title": "Explore the codebase", "readOnlyHint": true, "openWorldHint": false }`

**`fn call_explore_tool(params: &Value, cfg: &ExploreConfig, root: &Path) -> Outcome`** — new private helper:
- Extract `question` from `params["arguments"]["question"]`; missing → `Outcome::Ok(tool_text(&json!("missing required argument: question"), true))`.
- Construct `let client = OpenAiCompatClient::new(cfg)`.
- Call `run_explore(&question, root, cfg, &client)`.
- `Ok(answer)` → `Outcome::Ok(tool_text(&json!(answer.text), false))`.
- `Err(ExploreError::ProviderDown { url, detail })` → `Outcome::Ok(tool_text(&json!(format!("provider down ({}): {}; check the endpoint / run `grove config` / restart grove to pick up the structural fallback", url, detail)), true))`.
- `Err(ExploreError::Client(msg))` → `Outcome::Ok(tool_text(&json!(msg), true))`.

**Existing functions untouched**: `instructions()`, `tool_specs()`, `call_tool()`, `to_json()`, `missing()`, `outline_detail()`, `tool_text()`, and all unit tests in `mod tests`. The only internal calls that change are the two call sites of `handle()` (inside `serve()`) and the one call to `serve()` in `main.rs`.

### `cli/tests/cli.rs`

**New test** `explore_mode_unhealthy_provider_falls_back_to_standard_surface` (AC-6):
1. Create a `fixture("explore_fallback")` directory.
2. Write `.grove/explore.json` pointing to `http://127.0.0.1:1` (IANA Reserved port — guaranteed unreachable) with `model: "nomodel"`, `mode: "standard"`.
3. Spawn `grove serve <dir>` with `stdin: Stdio::piped()`, `stdout: Stdio::piped()`, `stderr: Stdio::piped()`.
4. Write two newline-delimited JSON-RPC messages to stdin: `initialize` (protocol `2025-06-18`) and `tools/list`. Close stdin.
5. `child.wait_with_output()` — server exits when stdin closes.
6. Parse each non-empty stdout line as JSON; find the response with `id: 2`.
7. Assert `result.tools.len() == 7` (structural fallback surface).
8. Assert `stderr` contains `"fallback"`.
9. Remove fixture dir.

---

## Data Model Changes

None. `ExploreConfig` is loaded from `.grove/explore.json` (existing T01 artefact) read-only. No new store fields.

---

## Testing Strategy

| Layer | Coverage |
|---|---|
| **Existing `mcp.rs` unit tests** | Must all pass unmodified — they call `call_tool`, `tool_specs`, `handle` (now with explicit `Surface::Standard` argument) directly. The refactor to `handle(method, params, surface)` adds one parameter; all existing test helpers must be updated to pass `&Surface::Standard`. |
| **Existing `tests/cli.rs`** | All existing CLI smoke tests pass unchanged. |
| **New integration test (AC-6)** | Covers unhealthy-startup fallback: exact tool count (7), stderr diagnostic note present, `initialize` succeeds. Deterministic — no real provider, port 1 fails immediately. |
| **Materiality** | Not material (no version bump required per task prompt). |

### Unit test update note

The existing `mod tests` in `mcp.rs` calls `handle(method, params)` (2 args). After the refactor to `handle(method, params, surface)` (3 args), all internal test helpers that invoke `handle` directly must be updated to pass `&Surface::Standard`. These are:

- `definition(args: Value) -> Value` (test helper, line 370)
- `call(name: &str, args: Value) -> Outcome` (line 537)
- `initialize_echoes_supported_version_else_default` (line 475)
- `ping_and_tools_list_and_notifications` (line 486)
- `every_tool_schema_is_client_registerable` (line 501)
- `unknown_method_is_method_not_found` (line 527)

---

## Acceptance Criteria

1. `grove serve` auto-detects explore mode when `.grove/explore.json` exists in the project root; `--explore` forces it; `--standard` suppresses it.
2. **Healthy startup** → `tools/list` returns exactly 1 tool named `explore` with a plain `{type:object, properties:{question:…}, required:["question"]}` inputSchema; `initialize` `instructions` describes delegation.
3. **Unhealthy startup** → `tools/list` returns the 7 structural tools byte-identical to default `--as mcp` output; a one-line fallback note appears on stderr; `initialize` always succeeds.
4. Mid-session `ExploreError::ProviderDown` → `isError: true` result with actionable message; server continues serving.
5. Without `.grove/explore.json` and no flags, serve behaviour is byte-identical to today (all existing unit tests and CLI smoke tests pass unmodified).
6. New deterministic integration test passes: unreachable `base_url` → fallback surface (7 tools) + stderr note.
7. Warning-clean, clippy-clean, `cargo test --release --locked` green.

---

## Operational Impact

- **Version bump**: not required (sprint-final flag; existing users see byte-identical behaviour by default).
- **Backward compat**: default-mode `grove serve` is byte-identical. The new `path` positional arg defaults to `.`, so `grove serve` (no args) works exactly as before.
- **Stdout / stderr discipline**: stdout carries JSON-RPC protocol only; all diagnostics (ready message, fallback note) go to stderr only.
