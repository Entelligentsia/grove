# VALIDATION REPORT — GROVE-S02-T02 (standalone review)

**Task:** OpenAI-compatible chat client + health probe (`core::explore::client`)
**Validator:** 🍵 grove QA Engineer
**Date:** 2026-07-02

---

**Verdict:** Approved

---

## Acceptance Criteria Verification

### AC-1 — `ChatClient` trait seam + `OpenAiCompatClient::new(&ExploreConfig)` via ureq/rustls

**PASS**

- `trait ChatClient { fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ClientError>; }` is present in `core/src/explore/client.rs` — confirmed by source inspection.
- `OpenAiCompatClient::new(&ExploreConfig)` implements `ChatClient`; it builds a `ureq::Agent` with explicit timeouts and POSTs to `{base_url}/chat/completions`. No new TLS/HTTP dependency: `git diff HEAD -- core/Cargo.toml` returns no output — `Cargo.toml` is unchanged.

### AC-2 — Wire model: messages, tools, tool_calls, tool_choice (non-streaming)

**PASS**

- `Role` enum (System/User/Assistant/Tool) serializes lowercase per OpenAI schema.
- `Message` carries `content`, `tool_calls`, `tool_call_id`, `name`; absent fields are `skip_serializing_if = "…"` so the wire body is lean.
- `ChatRequest` covers `messages`, `tools` (`Vec<Tool>`), `tool_choice` (`Option<Value>`), `temperature`. All optional fields omitted when unset.
- `ChatResponse` → `Vec<Choice>` → `message` — non-streaming (single response body).
- Test `chat_request_serializes_lean` verifies absent fields are not emitted.
- Test `request_with_tools_and_choice_serializes` verifies tool declarations and `tool_choice` round-trip.

### AC-3 — Works against Ollama and llama.cpp; dialect normalization contained in module

**PASS**

- `ToolCall` carries a custom `Deserialize` that normalizes `arguments` from either a JSON-encoded string (Ollama / OpenAI spec) or an already-parsed object (llama.cpp) into a uniform `serde_json::Value`.
- Test `provider_dialects_normalize_to_identical_tool_calls` deserializes `LLAMACPP_RESPONSE` and `OLLAMA_RESPONSE` fixtures and asserts `ta == tb` — same id, name, and arguments.
- No provider-specific logic visible in the public API; callers always receive a uniform `ToolCall`.

### AC-4 — `health_probe(&ExploreConfig) -> Result<(), HealthError>` with typed Unreachable/ModelMissing

**PASS**

- `health_probe` GETs `{base_url}/models`, parses `ModelsResponse`, and calls `model_available()`.
- Returns `HealthError::Unreachable` on transport/HTTP failure (with url + detail in the message) and `HealthError::ModelMissing` when the server answers but the model is absent (with model, url, available list).
- Both variants implement `fmt::Display` with actionable messages pointing at `explore.json`.
- Test `health_probe_against_unreachable_url_is_unreachable` verifies the error type and that the url names the `/models` endpoint.

### AC-5 — `ClientError` distinguishes connection errors from protocol/HTTP errors

**PASS**

- `ClientError::Connection` is emitted when `ureq::Error::Transport` is returned (server unreachable/refused/timed out) — this is the D3 shutdown signal.
- `ClientError::Http` is emitted for non-2xx `ureq::Error::Status` responses, carrying status code and body.
- `ClientError::Protocol` is emitted when the response body cannot be parsed as `ChatResponse`.
- `ClientError::is_connection()` provides a typed predicate for T03/T04 dispatch.
- Test `chat_against_unreachable_url_is_connection_error` verifies the `Connection` variant and that the url names `/chat/completions`.

### AC-6 — Tests: no HTTP mocks; unreachable URL → typed errors; serde round-trips for both providers

**PASS**

- All 12 `explore::client` tests run without HTTP mocks. `cargo test --release --locked -p grove-cst explore::client` → `12 passed; 0 failed`.
- Unreachable target is `127.0.0.1:1` (refused loopback port) — deterministic, no network required.
- Round-trip coverage: `chat_request_serializes_lean`, `request_with_tools_and_choice_serializes`, `tool_call_round_trips_through_canonical_shape`, `tool_message_carries_call_id`, `provider_dialects_normalize_to_identical_tool_calls`.
- Edge cases: `empty_and_null_arguments_normalize_to_object`, `unparseable_string_arguments_degrade_gracefully`, `response_without_tool_calls_is_plain_text`.
- Char-boundary regression: `truncate_does_not_panic_on_multibyte_boundary`.

### AC-7 — Warning-clean build, clippy clean, green tests; no "fastcontext" string

**PASS**

- `cargo test --release --locked -p grove-cst` → `107 lib tests + 1 doc-test: all passed`.
- `cargo clippy -p grove-cst -- -D warnings` → `Finished` with no warnings.
- `grep -r "fastcontext" core/src/explore/` → CLEAN.

---

## Plan-Review Advisories — Disposition

| # | Advisory | Status |
|---|----------|--------|
| 1 | `tool_call_id` on tool-role messages | ✅ `Message::tool(id, content)` sets `tool_call_id`; test `tool_message_carries_call_id` verifies serialization |
| 2 | Tolerant model matching for llama.cpp | ✅ `model_available()` matches exact, case-insensitive substring, file-stem, pre-`:` base; test `model_matching_is_tolerant` covers all patterns |
| 3 | Explicit request timeouts | ✅ `CONNECT_TIMEOUT` (10 s), `CHAT_TIMEOUT` (300 s), `PROBE_TIMEOUT` (30 s) via `ureq::AgentBuilder` |
| 4 | Error bodies in Protocol/Http | ✅ `ClientError::Http { body }` and `ClientError::Protocol { body }` carry response body (via char-boundary-safe `truncate()`) |
| 5 | Deterministic unreachable target | ✅ Tests use `127.0.0.1:1` — refused immediately and deterministically; no external network |

---

## Edge Cases Validated

- **Empty `tool_calls` vec omitted** — lean wire body.
- **`null`/empty tool arguments** — normalize to `{}`.
- **Unparseable string arguments** — degrade to the raw string (no panic).
- **Multi-byte UTF-8 at the 500-byte truncation boundary** — char-boundary-safe walk-back; regression test confirmed.
- **Empty model listing from server** — treated as available to avoid false `ModelMissing`.

---

## Regression Check

Full crate suite (`107 lib tests + 1 doc-test`) passed. No pre-existing tests were broken. Module is purely additive (`client.rs` new, `mod.rs` re-exports added only).

---

## Non-Blocking Notes (carried from Code Review)

- `model_available` substring matching can false-positive (e.g. `llama3` matching `llama31`). Accepted as tolerant-by-design; documented in implementation.
- Non-2xx on `/models` maps to `HealthError::Unreachable` with status detail — acceptable for T04; revisit if finer distinction needed.
