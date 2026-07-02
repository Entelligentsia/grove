# GROVE-S02-T03: Inner explorer agent loop with mode steering + tool gating (`core::explore`)

**Sprint:** GROVE-S02
**Estimate:** L
**Pipeline:** default

---

## Objective

The heart of the sprint: a bounded agent loop that lets a local model answer an
exploration question by driving grove's structural ops and an allowlisted set of
shell tools, with steering behavior selected by mode. This is what the single
`__explore` MCP tool (T04) executes per call.

## Acceptance Criteria

1. `core::explore::run_explore(question: &str, root: &Path, cfg: &ExploreConfig, client: &dyn ChatClient) -> Result<ExploreAnswer, ExploreError>`
   — an agent loop: system prompt + question → model → dispatch tool calls →
   feed results back → until the model produces a final answer.
2. **Inner toolset:** the 7 structural operations (internal calls to
   `grove_core::ops::…` — outline/symbols/source/check/callers/map/definition)
   plus shell tools from `cfg.allowed_tools` (e.g. `grove`, `find`, `grep`,
   `rg`), executed with the project root as cwd.
3. **Gating is enforced at schema AND execution level:** a tool not in the
   active toolset is (a) absent from the `tools` array sent to the model and
   (b) refused at dispatch with a corrective tool-result message if the model
   hallucinates it. Shell dispatch refuses any binary not on the allowlist —
   no shell interpolation (args passed as a vector).
4. **Modes are data, one loop:**
   - `standard` — full toolset, neutral steering (model chooses freely).
   - `aggressive` — full toolset, grove-first steering prompt (structural
     tools before text search).
   - `balanced` — harness-enforced two-phase: phase 1 exposes only structure
     verbs (map/symbols/outline/definition) plus a `submit_plan` tool; after N
     recon turns (default 2) only `submit_plan` remains; phase 2 exposes the
     full toolset with the committed plan injected as a standing hint.
5. Loop is bounded: max turns and max total tool-result bytes from config-level
   constants; exceeding them returns a best-effort answer, not an error.
6. `ClientError::Connection` from T02 propagates as
   `ExploreError::ProviderDown` — T04 maps it to server shutdown (D3).
7. Unit tests with a scripted fake `ChatClient` (no network): hallucinated
   tool call → corrective refusal, per-mode advertised-toolset assertions,
   balanced phase transition (recon tools close, `submit_plan` forced, plan
   hint present in phase 2), allowlist refusal of an off-list binary, turn
   bound respected.
8. Warning-clean, clippy clean, tests green; no "fastcontext" string; core
   stays clap-free.

## Context

Depends on **T02** (`ChatClient`). The mode semantics come straight from the
sidebench findings (see `SPRINT_REQUIREMENTS.md` Background): prompt-only
planning scored 0/4 with a 4B model — the balanced-mode phase gate MUST be
enforced by toolset construction, not by asking nicely. Small models also
hallucinate closed tools, hence criterion 3(b). Keep the loop boring: no
streaming, no parallel tool dispatch, no retries beyond one corrective message.

## Artifacts Involved

- New: `core/src/explore/agent.rs` (loop), `core/src/explore/toolset.rs`
  (tool schemas + dispatch + gating), `core/src/explore/steering.rs` (mode
  prompts as data).
- Edited: `core/src/explore/mod.rs` (public `run_explore`, `ExploreAnswer`,
  `ExploreError`).

## Operational Impact

- **Version bump:** not required (sprint-final).
- **Regeneration:** none.
- **Backward compat:** additive; the 7 structural ops' contracts untouched
  (explicitly out of scope per requirements).
