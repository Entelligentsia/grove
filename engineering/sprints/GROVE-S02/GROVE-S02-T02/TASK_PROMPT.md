# GROVE-S02-T02: OpenAI-compatible chat client + health probe (`core::explore::client`)

**Sprint:** GROVE-S02
**Estimate:** M
**Pipeline:** default

---

## Objective

Give the inner explorer its transport: a minimal OpenAI-compatible
chat-completions client (with tool calls) over the existing `ureq`/rustls stack,
working against Ollama (default) and llama.cpp server, plus a health probe that
T04 uses to gate MCP surfacing. All provider dialect quirks live in this one
module.

## Acceptance Criteria

1. `core/src/explore/client.rs` exposes:
   - `trait ChatClient { fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ClientError>; }`
     — the agent loop (T03) depends on the trait, never the concrete type.
   - `OpenAiCompatClient::new(&ExploreConfig)` implementing it via `ureq`
     (rustls; no new TLS/OpenSSL deps), POSTing `{base_url}/chat/completions`.
2. `ChatRequest`/`ChatResponse` model the OpenAI chat-completions schema subset
   needed: messages (system/user/assistant/tool), `tools` (function
   declarations with JSON-schema parameters), `tool_choice`, and response
   `tool_calls`. Non-streaming only.
3. Works against both **Ollama** (`/v1`) and **llama.cpp server** — dialect
   differences (e.g. tool-call encoding looseness) are normalized inside this
   module; nothing provider-specific leaks to callers.
4. `health_probe(&ExploreConfig) -> Result<(), HealthError>` verifies (a) the
   endpoint is reachable and (b) the configured model is available, with typed
   errors distinguishing **unreachable** vs **model-missing**, each carrying an
   actionable message.
5. `ClientError` distinguishes connection errors from protocol/HTTP errors —
   T03/T04 map connection errors to server shutdown (D3).
6. Tests (no HTTP mocks, per project convention — test to the
   error-before-connect boundary): unreachable URL → typed unreachable error;
   request/response serde round-trips including tool-call payloads from both
   Ollama and llama.cpp sample JSON fixtures.
7. Warning-clean build, clippy clean, tests green; no "fastcontext" string.

## Context

Depends on **T01** (`ExploreConfig`, `Provider`). Sprint decisions D2 (OpenAI
compat; Ollama default; llama.cpp supported) and D3 (typed connection errors
feed the shutdown semantics). The stack checklist requires ureq-rustls — do not
introduce a second HTTP client. Sample tool-call responses for fixtures can be
captured once from live Ollama/llama.cpp and committed as JSON.

## Artifacts Involved

- New: `core/src/explore/client.rs`, JSON fixtures under `core/src/explore/`
  test data (inline or `include_str!`).
- Edited: `core/src/explore/mod.rs` (module wiring), `core/Cargo.toml` only if
  a serde helper is needed (prefer none).

## Operational Impact

- **Version bump:** not required (sprint-final).
- **Regeneration:** none.
- **Backward compat:** additive; no existing surface touched.
