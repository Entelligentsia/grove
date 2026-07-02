# PROGRESS — GROVE-S02-T02: OpenAI-compatible chat client + health probe (`core::explore::client`)

## Summary of Changes

### Code-review revision (this cycle) — BLOCKING defect fixed

- **`truncate()` panicked on non-ASCII bodies.** The original implementation used `&s[..500]` which panics when byte 500 falls inside a multi-byte UTF-8 character. The fixed version walks back to the nearest char boundary:

  ```rust
  let mut cut = MAX;
  while cut > 0 && !s.is_char_boundary(cut) {
      cut -= 1;
  }
  format!("{}… ({} bytes)", &s[..cut], s.len())
  ```

- **Regression test added** — `truncate_does_not_panic_on_multibyte_boundary`: builds a string of 499 ASCII chars + 50 `é` (2 bytes each) so the 500-byte cutoff lands mid-codepoint; asserts no panic and that the prefix is on a char boundary ≤ 500 bytes.

### Review-plan advisories — disposition (carried from first implementation)

- **#1 tool_call_id** — `Message::tool(id, content)` sets `tool_call_id`, per OpenAI schema for tool-result turns. Test `tool_message_carries_call_id` verifies serialization.
- **#2 tolerant model matching** — `model_available()` matches exact string, case-insensitive substring, and strips `.gguf`/path/alias patterns so llama.cpp model IDs don't produce false `ModelMissing` errors. Test `model_matching_is_tolerant` covers all patterns.
- **#3 timeouts** — explicit `timeout_connect` (10 s), chat overall timeout (300 s), probe timeout (30 s) via `CONNECT_TIMEOUT`, `CHAT_TIMEOUT`, `PROBE_TIMEOUT` constants.
- **#4 error bodies** — `Http`/`Protocol` carry the response body (truncated via the now-safe `truncate()`) for actionable messages.
- **#5 deterministic unreachable target** — tests point at `127.0.0.1:1` (a refused loopback port), not an external host, so CI never flakes.

## Test Evidence

### `cargo test --release --locked -p grove-cst explore::client` (module)

```
    Finished `release` profile [optimized] target(s) in 0.53s
     Running unittests src/lib.rs (target/release/deps/grove_core-a7075cf756ab5e11)

running 12 tests
test explore::client::tests::model_matching_is_tolerant ... ok
test explore::client::tests::truncate_does_not_panic_on_multibyte_boundary ... ok
test explore::client::tests::empty_and_null_arguments_normalize_to_object ... ok
test explore::client::tests::chat_request_serializes_lean ... ok
test explore::client::tests::tool_message_carries_call_id ... ok
test explore::client::tests::tool_call_round_trips_through_canonical_shape ... ok
test explore::client::tests::response_without_tool_calls_is_plain_text ... ok
test explore::client::tests::request_with_tools_and_choice_serializes ... ok
test explore::client::tests::provider_dialects_normalize_to_identical_tool_calls ... ok
test explore::client::tests::unparseable_string_arguments_degrade_gracefully ... ok
test explore::client::tests::chat_against_unreachable_url_is_connection_error ... ok
test explore::client::tests::health_probe_against_unreachable_url_is_unreachable ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 95 filtered out; finished in 0.01s
```

### `cargo test --release --locked -p grove-cst` (full crate suite)

```
    Finished `release` profile [optimized] target(s) in 0.06s
     Running unittests src/lib.rs (target/release/deps/grove_core-a7075cf756ab5e11)

running 107 tests
test engine::tests::extract_imports_javascript_named_and_aliased ... ok
test engine::tests::extract_imports_python_named_and_aliased ... ok
test engine::tests::enclosing_function_at_qualifies_method_with_its_type ... ok
[... 104 further tests all ok ...]
test explore::client::tests::truncate_does_not_panic_on_multibyte_boundary ... ok
test explore::client::tests::chat_against_unreachable_url_is_connection_error ... ok
test explore::client::tests::health_probe_against_unreachable_url_is_unreachable ... ok

test result: ok. 107 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s

   Doc-tests grove_core

running 1 test
test core/src/lib.rs - (line 28) - compile ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
```

### `cargo clippy -p grove-cst -- -D warnings`

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.21s
```

(No warnings — exit 0.)

### `fastcontext` guard (plan verification step)

```
$ grep -r "fastcontext" core/src/explore/
(no output — string not present)
```

### grove syntax check

`mcp__grove__check` on `core/src/explore/client.rs` — 0 ERROR / MISSING nodes.

## Files Changed

| File | Change |
| --- | --- |
| `core/src/explore/client.rs` | **New file** — wire model (`Role`, `Message`, `ToolCall`, `Tool`, `ChatRequest`, `ChatResponse`), `ChatClient` trait, `OpenAiCompatClient` over ureq/rustls, `ClientError`, `health_probe`, `HealthError`, dialect normalization, 12 inline `#[cfg(test)]` tests. Includes char-boundary-safe `truncate()` + regression test. |
| `core/src/explore/mod.rs` | Added `pub mod client;` + re-exports of the full public surface (`ChatClient`, `OpenAiCompatClient`, `ChatRequest`, `ChatResponse`, `Message`, `Role`, `Tool`, `ToolCall`, `ClientError`, `health_probe`, `HealthError`) so T03/T04 can `use grove_core::explore::...`. |

## Acceptance Criteria — status

| # | Criterion | Status |
|---|-----------|--------|
| 1 | `trait ChatClient` with `chat()` seam | ✅ |
| 2 | `OpenAiCompatClient::new(&ExploreConfig)` over existing ureq/rustls | ✅ |
| 3 | Wire model: messages, tools, tool_calls, tool_choice | ✅ |
| 4 | Dialect normalization: Ollama string-args ↔ llama.cpp object-args → identical `ToolCall` | ✅ |
| 5 | `health_probe` typing: `HealthError::Unreachable` vs `HealthError::ModelMissing` | ✅ |
| 6 | `ClientError` Connection vs Protocol/HTTP split | ✅ |
| 7 | No HTTP mocks; tests use `127.0.0.1:1`; `cargo test` + clippy green | ✅ |
