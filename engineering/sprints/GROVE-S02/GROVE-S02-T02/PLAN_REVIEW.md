# PLAN REVIEW — GROVE-S02-T02: OpenAI-compatible chat client + health probe (standalone review)

**Verdict:** Approved

## Independent Verification

Claims in the plan were checked against the actual repository, not the plan text:

- **T01 dependency real:** `core/src/explore/config.rs` exists with `ExploreConfig { provider, base_url, model, mode, allowed_tools }` and `Provider` — exactly the fields the plan says it reads (`base_url`, `model`, `provider`). No config change needed; plan's "Data Model Changes: none" claim is accurate.
- **No new HTTP/TLS dependency claim holds:** `core/Cargo.toml` already declares `ureq = "2"` (commented "blocking, rustls"), used in `core/src/fetch.rs`. Stack checklist § Dependencies requires ureq-rustls / no OpenSSL — plan complies.
- **`base_url` includes `/v1`** (doc comment: `http://localhost:11434/v1`), so `{base_url}/chat/completions` and `{base_url}/models` are the correct OpenAI-compat paths for both Ollama and llama.cpp server.
- **Testing convention matches:** stack checklist § Testing requires inline `#[cfg(test)] mod tests` and `cargo test --release --locked`; plan specifies both. No-HTTP-mock, error-before-connect boundary matches project convention and the task prompt's AC6 verbatim.
- **Explore module wiring:** `core/src/explore/` currently holds `config.rs` + `mod.rs`; adding `pub mod client;` + re-exports is the correct, additive integration point.

## Coverage vs Task Acceptance Criteria

| AC | Covered by plan |
|----|-----------------|
| 1. `ChatClient` trait + `OpenAiCompatClient::new(&ExploreConfig)` via ureq | ✅ Approach §2 |
| 2. Wire model: messages, tools, tool_choice, tool_calls, non-streaming | ✅ Approach §1 |
| 3. Ollama + llama.cpp dialect normalization contained in module | ✅ Approach §3 (tolerant `arguments` object-vs-string handling) |
| 4. `health_probe` typed Unreachable vs ModelMissing, actionable messages | ✅ Approach §4 |
| 5. `ClientError` Connection vs Protocol/HTTP (D3 shutdown signal) | ✅ Approach §4 (`ureq::Error::Transport` vs `Status` mapping) |
| 6. Tests: no mocks, fixtures from both providers, unreachable → typed error | ✅ Testing Strategy |
| 7. Clippy clean, tests green, no "fastcontext" | ✅ Acceptance Criteria + verify commands |

Security surface is minimal (localhost inference endpoint, no credentials, no user-supplied URL interpolation beyond config); typed error separation is the main safety-relevant concern and is handled per D3.

## Advisory Notes (non-blocking)

1. **`tool` role needs `tool_call_id`.** The OpenAI schema requires `tool_call_id` on tool-role messages so the model can correlate results with calls. The plan says roles are "represented" but doesn't name this field — ensure `Message` carries it (optional, `skip_serializing_if`) or T03's feed-back loop will fail against stricter servers.
2. **llama.cpp `/models` id mismatch risk.** llama.cpp's `/v1/models` often reports the model file path/alias, not the Ollama-style tag the user configured. A strict equality check could yield false `ModelMissing`. Consider a tolerant match (substring/alias) or provider-aware leniency — this is dialect knowledge that per AC3 must stay inside this module.
3. **Request timeout.** ureq's defaults plus a slow local LLM generation can produce long or premature-timeout behaviour. Decide the agent timeout explicitly (generous read timeout or none) and ensure a read timeout maps to `Connection` only if that is the intended D3 semantics — a mid-generation timeout is arguably not "server gone".
4. **HTTP-status error bodies.** Both providers return useful JSON error bodies on 4xx; capturing the body into the `Protocol`/`Http` error message makes it actionable (mirrors the AC4 "actionable message" spirit).
5. **Unreachable-URL test flakiness.** Prefer a reserved invalid TLD or `127.0.0.1:1` style port over an arbitrary "closed port" to keep the connection-error test deterministic across CI environments.

None of the above changes the plan's structure; they are implementation-detail guardrails for the engineer.
