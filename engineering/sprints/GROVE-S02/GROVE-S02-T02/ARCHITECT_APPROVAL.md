# Architect Approval — GROVE-S02-T02

OpenAI-compatible chat client (with tool calls) + health probe in `core::explore::client`.

**Verdict:** Approved

## Rationale

The implementation is architecturally sound and consistent with the grove stack:

- **No new dependency.** `core/Cargo.toml` shows zero diff. The client is built on the
  existing `ureq 2 (rustls)` HTTP client already vendored for `grove fetch` and documented
  in `architecture/stack.md § Core Dependencies`. `serde_json::Value` covers arbitrary tool
  schemas. This is the preferred outcome — no expansion of the dependency surface.
- **Correct abstraction seam.** `ChatClient` is a trait; T03/T04 depend on the trait, never
  the concrete `OpenAiCompatClient`. This keeps the agent loop testable and provider-swappable.
- **Typed error boundary serves the runtime.** `ClientError::Connection` (from `ureq::Error::Transport`)
  is distinguished from `Protocol`/`Http` so the T03 agent loop can map connection loss to the
  D3 shutdown signal. `HealthError::{Unreachable, ModelMissing}` carry actionable messages.
- **Dialect normalization is correctly localized.** Ollama (string-args) vs llama.cpp (object-args)
  tool-call divergence is absorbed entirely inside this module, presenting a uniform `ToolCall`
  to callers — the right place for provider-specific tolerance.
- **Blocking defect resolved.** The char-boundary-safe `truncate()` fix (walk-back before slice)
  eliminates the multi-byte UTF-8 panic, with a genuine regression test.

## Cross-Cutting Concerns

- Scope is clean: only `core/src/explore/client.rs` (new) and `core/src/explore/mod.rs`
  (module wiring + public re-exports) affect the code surface. No other module is touched.
- All new types are in-memory transport types — no persisted schema or config migration.
  The task only *reads* `ExploreConfig` (T01). No consumer breakage.

## Deployment Notes

- No operational impact: no new runtime dependency, no migration, no config change.
- Additive, non-material change — no version bump required.
- Runtime behavior depends on a reachable OpenAI-compatible endpoint (Ollama / llama.cpp);
  failure modes are typed and surfaced, not swallowed.

## Follow-Up Items (future sprints)

- `model_available()` substring matching can theoretically false-positive (e.g. `llama3` vs
  `llama31`). Documented as tolerant-by-design; revisit only if a stricter match is needed.
- Non-2xx on `/models` currently maps to `HealthError::Unreachable` with status detail.
  Acceptable now; if T04 needs to distinguish "server up but erroring" from "unreachable",
  introduce a finer variant then.
- Non-streaming only by design. Streaming support, if ever required, is a separate future task.
