# PLAN — GROVE-S02-T02: OpenAI-compatible chat client + health probe (`core::explore::client`)

🌱 *grove Engineer*

**Task:** GROVE-S02-T02
**Sprint:** GROVE-S02
**Estimate:** M

---

## Objective

Give the inner explorer its transport layer: a minimal, non-streaming
OpenAI-compatible chat-completions client (with tool-call support) built on the
existing `ureq`/rustls stack, plus a health probe that later tasks (T04) use to
gate MCP surfacing. The client works against both Ollama (default) and
llama.cpp server, normalizing every provider dialect quirk inside this one
module so nothing provider-specific leaks to callers. Errors are typed to
distinguish connection failures (which T03/T04 map to server shutdown per D3)
from protocol/HTTP failures, and the health probe distinguishes an unreachable
endpoint from a missing model.

## Approach

Introduce a new `core/src/explore/client.rs` module wired into
`core/src/explore/mod.rs`. The design is layered into four concerns:

1. **Wire model** — serde structs modelling the OpenAI chat-completions request
   and response subset we need: `ChatRequest` (messages, tools, tool_choice)
   and `ChatResponse` (choices → message → optional tool_calls). Message roles
   (system / user / assistant / tool) are represented so the T03 agent loop can
   build multi-turn conversations and feed tool results back. Tool declarations
   carry a name, description, and a JSON-schema `parameters` value
   (`serde_json::Value`, so arbitrary schemas pass through untouched). All
   request-side optional fields are `skip_serializing_if` so we emit a lean body
   both providers accept.

2. **Transport trait** — `trait ChatClient { fn chat(&self, req: ChatRequest)
   -> Result<ChatResponse, ClientError>; }`. The agent loop depends only on this
   trait, never the concrete type, so T03 can substitute a fake in tests. The
   concrete `OpenAiCompatClient::new(&ExploreConfig)` captures the base URL and
   model from config, POSTs to `{base_url}/chat/completions` via `ureq` (reusing
   the same blocking rustls client already used in `fetch.rs` — no new HTTP or
   TLS dependency), and normalizes the response.

3. **Dialect normalization** — Ollama and llama.cpp both speak OpenAI-compatible
   JSON but differ in tool-call encoding looseness (e.g. `arguments` arriving as
   an already-parsed object vs. a JSON-encoded string; presence/absence of
   `tool_calls` when there is none). A single deserialization path with a
   tolerant custom handling for the `arguments` field absorbs these differences
   so callers always receive a uniform `ToolCall` shape. This is the only place
   provider knowledge lives.

4. **Typed errors + health probe** — `ClientError` is an enum distinguishing
   `Connection` (transport unreachable / refused / timeout — the D3 shutdown
   signal) from `Protocol`/`Http` (reachable but bad status or unparseable
   body). `health_probe(&ExploreConfig) -> Result<(), HealthError>` performs a
   lightweight reachability + model-availability check (GET `{base_url}/models`,
   the OpenAI-standard model listing both providers expose), with `HealthError`
   distinguishing `Unreachable` from `ModelMissing`, each carrying an actionable
   message naming the endpoint or model. Connection classification is derived by
   inspecting the `ureq::Error` variant (`Transport` → connection; `Status` →
   http/protocol).

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `core/src/explore/client.rs` | **New** — wire model, `ChatClient` trait, `OpenAiCompatClient`, `ClientError`, `health_probe`, `HealthError`, inline `#[cfg(test)] mod tests` + JSON fixtures | The task deliverable |
| `core/src/explore/mod.rs` | Add `pub mod client;` and re-export the public surface (`ChatClient`, `OpenAiCompatClient`, `ChatRequest`, `ChatResponse`, `Message`, `ToolCall`, `ClientError`, `health_probe`, `HealthError`) | Module wiring so T03/T04 can `use grove_core::explore::...` |
| `core/Cargo.toml` | Only if a serde helper is required — **preferred: no change** | Task guidance prefers introducing no new dependency; `serde_json::Value` already covers arbitrary tool schemas |

Sample tool-call response fixtures (one Ollama-shaped, one llama.cpp-shaped)
are embedded inline as string literals inside the test module (no separate
files needed for such small payloads; matches the config.rs precedent of inline
fixture JSON).

## Data Model Changes

No changes to the persisted `.grove/explore.json` schema or `ExploreConfig`.
This task only **reads** `ExploreConfig` (`base_url`, `model`, `provider`). All
new types (`ChatRequest`, `ChatResponse`, `Message`, `ToolCall`, `Tool`,
`ClientError`, `HealthError`) are in-memory transport types, not persisted, so
there is no store or config-file migration.

## Testing Strategy

Per project convention there are **no HTTP mocks** — tests exercise serde
round-trips and the error-before-connect boundary only:

- **Serde round-trips:** `ChatRequest` serializes to the expected OpenAI body
  shape (roles, tools, tool_choice present; optional fields omitted when unset).
- **Response deserialization from fixtures:** an Ollama-shaped
  chat-completions response with `tool_calls` and a llama.cpp-shaped one both
  deserialize into the identical normalized `ToolCall` structure (proving
  dialect normalization). Include a case where `arguments` is a JSON-encoded
  string and one where it is an object.
- **Connection-error classification:** a client pointed at an unreachable URL
  (e.g. a closed port on localhost / an invalid host) returns
  `ClientError::Connection` — verifying the D3 mapping without a live server.
- **Health probe error typing:** `health_probe` against an unreachable endpoint
  returns `HealthError::Unreachable` with a message naming the endpoint.
- Commands: `cargo test --release --locked`, `cargo clippy -- -D warnings`.

## Acceptance Criteria

- [ ] `core/src/explore/client.rs` exposes `trait ChatClient` with
      `fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ClientError>`.
- [ ] `OpenAiCompatClient::new(&ExploreConfig)` implements `ChatClient` via
      `ureq` (rustls), POSTing `{base_url}/chat/completions`; no new TLS/HTTP dep.
- [ ] `ChatRequest`/`ChatResponse` model messages (system/user/assistant/tool),
      `tools` (function decls with JSON-schema params), `tool_choice`, and
      response `tool_calls`; non-streaming only.
- [ ] Works against Ollama and llama.cpp; tool-call dialect differences are
      normalized inside the module (proven by both fixtures deserializing
      identically).
- [ ] `health_probe(&ExploreConfig) -> Result<(), HealthError>` distinguishes
      **unreachable** vs **model-missing**, each with an actionable message.
- [ ] `ClientError` distinguishes connection errors from protocol/HTTP errors.
- [ ] Tests (no HTTP mocks): unreachable URL → typed connection/unreachable
      error; serde round-trips including tool-call payloads from both provider
      fixtures.
- [ ] `cargo test --release --locked` green; `cargo clippy -- -D warnings`
      clean; no `"fastcontext"` string anywhere.

## Operational Impact

- **Version bump:** not required (sprint-final; additive internal module).
- **Regeneration:** none.
- **Backward compatibility:** additive — a new module and re-exports only; no
  existing public surface is modified, so no consumer breaks.
- **Distribution:** no user-facing action; the client is not yet wired to any
  command (T03/T04 consume it).
- **Materiality:** NOT material for versioning — new code with no change to an
  existing shipped command, hook, tool spec, workflow, or store/config schema.
