# Code Review — GROVE-S02-T02 (standalone re-review)

**Verdict:** Approved

OpenAI-compatible chat client + health probe in `core::explore::client`. This is
a re-review following the prior **Revision Required** verdict, whose single
blocking item was a `truncate()` byte-slice panic. That defect is now fixed and
independently verified.

## Blocking item from prior review — RESOLVED

`truncate()` previously did `&s[..500]`, which panics when a multi-byte UTF-8
character straddles byte 500. The new implementation walks back to the nearest
char boundary before slicing:

```rust
let mut cut = MAX;
while cut > 0 && !s.is_char_boundary(cut) { cut -= 1; }
format!("{}… ({} bytes)", &s[..cut], s.len())
```

- Correct: `is_char_boundary` guarantees `&s[..cut]` never bisects a codepoint.
- Regression test `truncate_does_not_panic_on_multibyte_boundary` reproduces the
  exact failing case (499 ASCII + 50 `é` straddling byte 500) and passes.
- The path matters: `truncate()` runs on arbitrary server bodies inside
  `ClientError::Http` Display and the health-probe error path, so a panic there
  would convert a typed, recoverable error into a crash — defeating the D3
  shutdown-signal design. Now safe.

## Independent verification

- `cargo test --release --locked -p grove-cst explore::client` → 12 passed.
- `cargo test --release --locked -p grove-cst --lib` → 107 passed, 0 failed.
- `cargo clippy -p grove-cst -- -D warnings` → clean, no warnings.
- `rg fastcontext core/src/explore/` → no match (naming constraint honoured).
- `git diff core/src/explore/mod.rs` confirms `pub mod client;` + full re-export
  surface (ChatClient, ChatRequest/Response, Message, ToolCall, Tool, Role,
  ClientError, HealthError, health_probe, OpenAiCompatClient) for T03/T04.

## Acceptance criteria (all met)

1. `ChatClient` trait is the T03/T04 seam; concrete `OpenAiCompatClient` behind it. ✓
2. Built on existing `ureq` + rustls — no new dependency (Cargo.toml unchanged). ✓
3. Full non-streaming wire model: messages (system/user/assistant/tool), tools
   with JSON-schema params, tool_choice, response tool_calls. ✓
4. Dialect normalization: llama.cpp object-args and Ollama string-args both
   deserialize to an identical `ToolCall`; re-serialization emits the canonical
   OpenAI string-args shape. Verified by
   `provider_dialects_normalize_to_identical_tool_calls` and the round-trip test. ✓
5. `ClientError` separates `Connection` (D3 signal) from `Http`/`Protocol`/`Encode`,
   derived from `ureq::Error` `Transport` vs `Status`. ✓
6. `health_probe` GETs `{base_url}/models`; `HealthError::Unreachable` vs
   `ModelMissing` with actionable messages. ✓
7. Mock-free tests using a deterministic closed port (127.0.0.1:1). ✓

## Plan-review advisories (all dispositioned)

- tool_call_id carried on `tool` messages (`Message::tool`, tested). ✓
- Tolerant model matching for `.gguf`/alias/tag-base (`model_available`, tested). ✓
- Explicit timeouts: connect 10s, chat 300s, probe 30s. ✓
- HTTP error-response bodies carried in `ClientError::Http`. ✓
- Deterministic unreachable target (127.0.0.1:1) for connection tests. ✓

## Advisory notes (non-blocking)

1. `model_available` uses `id.contains(want)`, so `want="llama3"` matches a
   served `llama31` (false positive). Documented as tolerant-by-design; acceptable
   given the goal is avoiding false `ModelMissing`. Worth a comment if strict
   matching is ever needed for T03's UX.
2. `health_probe` treats an empty `/models` listing as a match. Reasonable for
   servers that report no models while one is loaded; note it means the probe
   can't catch a genuinely empty server. Acceptable for now.
3. A non-2xx on `/models` maps to `HealthError::Unreachable` (with status in the
   detail) rather than a distinct variant — fine for the current error surface.

Clean, well-documented, meaningfully tested. Approved.
