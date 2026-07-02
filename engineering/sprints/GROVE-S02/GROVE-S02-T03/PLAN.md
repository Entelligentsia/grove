# PLAN — GROVE-S02-T03
# Inner Explorer Agent Loop with Mode Steering + Tool Gating (`core::explore`)

## Objective

Implement the bounded agent loop (`run_explore`) inside `core::explore` that drives
a local OpenAI-compatible model through grove's structural ops and allowlisted shell
tools, with mode-selected steering and harness-enforced tool gating.

---

## Approach

The implementation is decomposed into three focused modules, keeping each concern
isolated, then wired together through `mod.rs`. No streaming, no parallel dispatch,
no retries beyond one corrective message per hallucinated tool — per the sidebench
context rule ("keep the loop boring").

### Module Map

```
core/src/explore/
  agent.rs      — outer loop: message history, turn/byte accounting, phase machine
  toolset.rs    — Tool schema registry, active-toolset construction, dispatch, gating
  steering.rs   — Mode steering data: per-mode system-prompt text (pure data, no logic)
  mod.rs        — pub re-exports: run_explore, ExploreAnswer, ExploreError
```

---

## Design Details

### Public API (`mod.rs` additions)

```rust
// Public types surfaced from agent.rs
pub use agent::{run_explore, ExploreAnswer, ExploreError};
```

`run_explore` signature (matching AC-1):
```rust
pub fn run_explore(
    question: &str,
    root: &Path,
    cfg: &ExploreConfig,
    client: &dyn ChatClient,
) -> Result<ExploreAnswer, ExploreError>
```

### `ExploreAnswer` and `ExploreError`

```rust
pub struct ExploreAnswer {
    pub text: String,   // model's final answer text
    pub turns: usize,   // number of turns consumed
    pub truncated: bool, // true if turn/byte limit was hit
}

pub enum ExploreError {
    // ClientError::Connection maps here (AC-6)
    ProviderDown { url: String, detail: String },
    // Other ClientError variants
    Client(String),
}
```

Turn/byte limits produce `Ok(ExploreAnswer { truncated: true, … })` — a best-effort
answer, never an error variant (AC-5).

### `steering.rs` — Mode Prompts as Data

Contains:
- `pub fn system_prompt(mode: Mode) -> &'static str` — returns a static string
  per mode:
  - `Standard`: neutral system prompt (model chooses freely)
  - `Aggressive`: grove-first steering prompt urging structural tools before
    `grep`/`rg`
  - `Balanced`: phase-1 recon prompt (tells model to use structure tools and
    call `submit_plan` when ready); phase-2 prompt that prepends the committed
    plan as a standing hint

No runtime state; all mode behavior is encoded in returned text.

### `toolset.rs` — Schema Registry, Gating, Dispatch

**Constants (module-level):**
```rust
pub const MAX_TURNS: usize = 25;
pub const MAX_TOOL_RESULT_BYTES: usize = 128 * 1_024; // 128 KiB total
pub const BALANCED_RECON_TURNS: usize = 2; // from cfg; default value here
```

**Structural op names (the 7 grove ops):**
```rust
const STRUCTURAL_OPS: &[&str] = &[
    "outline", "symbols", "source", "check", "callers", "map", "definition",
];
const RECON_OPS: &[&str] = &["map", "symbols", "outline", "definition"];
```

**`build_full_toolset(root, allowed_shell) -> Vec<Tool>`**
Returns JSON-schema `Tool` structs for all 7 structural ops + each binary in
`allowed_shell` (e.g. `grove`, `find`, `grep`, `rg`). Tool names match op
function names.

**`build_recon_toolset() -> Vec<Tool>`**
Returns `Tool` structs for `RECON_OPS` only, plus a synthetic `submit_plan`
tool (takes one `plan: String` argument). Used in Balanced phase 1.

**`build_submit_only_toolset() -> Vec<Tool>`**
Returns only `submit_plan`. Forces the model to commit its plan after N recon turns.

**`dispatch_tool(name, args_json, root, allowed_shell) -> String`**
- Matches `name` against structural ops → calls `grove_core::ops::*` directly
  (no subprocess); serializes result to JSON string.
- Matches `name` against `allowed_shell` → spawns with `std::process::Command`,
  args passed as a `Vec<String>` (no shell interpolation), cwd = root; captures
  stdout+stderr; truncates to a per-call byte cap.
- Unrecognized name → returns an error string used as the corrective tool-result
  message.

**`is_in_toolset(name, active: &[Tool]) -> bool`**
Returns whether `name` appears in the active toolset's function names.

### `agent.rs` — The Loop

```
Messages: Vec<Message>
  [0] system  — mode system prompt (or phase-2 system prompt with plan)
  [1] user    — the exploration question
  [n] assistant + tool results  — appended each turn
```

**Phase machine (Balanced mode only):**
```
Phase::Recon(turn: usize)  — exposes RECON_OPS + submit_plan
Phase::ForceSubmit          — exposes submit_plan only (after N recon turns)
Phase::Execute(plan: String) — full toolset, plan injected into system message
```

For `Standard` and `Aggressive`, phase is always `Execute` with no machine.

**Turn loop logic:**

```
loop:
  1. Build active toolset from current phase/mode
  2. Build ChatRequest from messages + active toolset
  3. For Aggressive: also set tool_choice hint (prefer grove ops)
  4. client.chat(req) → response or error
     - ClientError::Connection → return Err(ExploreError::ProviderDown)
     - Other ClientError → return Err(ExploreError::Client(...))
  5. Append assistant message to history
  6. If response.choices[0].message.tool_calls is empty:
     → extract content as answer text; break loop
  7. For each tool_call in response:
     a. If name not in active toolset:
        → append corrective tool-result ("tool '{name}' is not available…")
        → continue (one corrective per hallucination)
     b. Else: dispatch_tool(name, args, root, allowed_shell)
        → append tool result message
        → accumulate byte count
  8. Check Balanced phase transition:
     - In Recon: if submit_plan called → Phase::Execute(plan_text), rebuild system msg
     - In Recon: if turn count >= BALANCED_RECON_TURNS → Phase::ForceSubmit
     - In ForceSubmit: if submit_plan called → Phase::Execute(plan_text)
  9. Increment turn counter
  10. If turns >= MAX_TURNS OR total_tool_bytes >= MAX_TOOL_RESULT_BYTES:
      → return Ok(ExploreAnswer { text: best_effort_text, turns, truncated: true })
```

`best_effort_text` when truncated: last assistant content seen, or a canned message
if no content was produced yet.

**Shell dispatch safety:**
`std::process::Command::new(binary).args(&args_vec).current_dir(root)` — no
`.arg("sh").arg("-c")` path. Binary name is validated against `allowed_shell` before
`Command::new` is ever called.

---

## Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `core/src/explore/agent.rs` | Create | `run_explore` function, `ExploreAnswer`, `ExploreError`, loop + phase machine |
| `core/src/explore/toolset.rs` | Create | Tool schema registry, toolset builders, dispatch, gating |
| `core/src/explore/steering.rs` | Create | Per-mode system prompt text (static data) |
| `core/src/explore/mod.rs` | Edit | Add `pub mod agent`, `pub mod toolset`, `pub mod steering`; re-export `run_explore`, `ExploreAnswer`, `ExploreError` |

---

## Data Model Changes

No new persistent state. `ExploreConfig.allowed_tools` and `ExploreConfig.mode`
(both from T01) drive all gating decisions. No schema changes.

---

## Testing Strategy

All tests live in `#[cfg(test)]` blocks within the new modules. Tests use a
**scripted fake `ChatClient`** (a struct with a `Vec<ChatResponse>` queue that
pops one per `chat()` call). No network, no filesystem writes, no subprocess
execution (structural op dispatch mocked via test helpers).

### Test Coverage (AC-7)

| Test | Verifies |
|------|----------|
| `hallucinated_tool_returns_corrective_refusal` | Model calls unknown tool → corrective tool-result injected; loop continues |
| `standard_toolset_contains_all_seven_ops` | `build_full_toolset` includes all 7 structural op names |
| `aggressive_toolset_same_as_standard` | `build_full_toolset` same set (steering is prompt-only, not schema-level) |
| `balanced_phase1_toolset_recon_plus_submit_plan` | `build_recon_toolset` contains exactly RECON_OPS + `submit_plan` |
| `balanced_phase_transitions_after_n_recon_turns` | After `BALANCED_RECON_TURNS` without `submit_plan`, toolset becomes `submit_plan`-only |
| `balanced_phase2_has_plan_hint_in_system_message` | After `submit_plan` called, phase-2 system message includes plan text |
| `shell_binary_not_in_allowlist_is_refused` | `dispatch_tool("curl", …)` with empty allowlist returns refusal string |
| `turn_bound_terminates_loop` | Scripted client returns tool_calls indefinitely; loop exits at `MAX_TURNS` with `truncated: true` |
| `byte_bound_terminates_loop` | Scripted client's tool result accumulates; loop exits when total_tool_bytes exceeded |
| `connection_error_maps_to_provider_down` | `client.chat` returns `ClientError::Connection` → `Err(ExploreError::ProviderDown)` |

---

## Acceptance Criteria Mapping

| AC | How Met |
|----|---------|
| AC-1 | `run_explore` signature matches exactly; lives in `agent.rs`, re-exported from `mod.rs` |
| AC-2 | `toolset.rs` maps all 7 `grove_core::ops::*` functions + `cfg.allowed_tools` shell binaries |
| AC-3 | `build_*_toolset` omits non-active tools from `tools` array; `dispatch_tool` returns corrective string for unknown names |
| AC-4 | Phase machine in `agent.rs` selects toolset per mode; Balanced harness enforces recon phase limit |
| AC-5 | Turn+byte counters produce `Ok(ExploreAnswer { truncated: true })` — no error |
| AC-6 | `ClientError::Connection` arm maps to `ExploreError::ProviderDown` |
| AC-7 | Unit tests cover all 7 scripted fake-client scenarios |
| AC-8 | No `fastcontext`, no `clap`, no warnings; `cargo clippy -- -D warnings` clean |

---

## Operational Impact

- **Version bump:** not required (sprint-final additive module).
- **Regeneration:** none.
- **Backward compatibility:** additive only — no changes to the 7 structural ops' contracts or the `client` / `config` public APIs.
- **No new Cargo.toml dependencies:** `std::process::Command` handles shell dispatch;
  `grove_core::ops` handles structural ops; `serde_json::Value` for arg parsing is
  already available.
