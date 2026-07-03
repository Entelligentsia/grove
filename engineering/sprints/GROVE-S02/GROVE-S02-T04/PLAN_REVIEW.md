# PLAN_REVIEW — GROVE-S02-T04: `grove serve` explore mode (standalone review)

**Verdict:** Approved

## Independent verification performed

I did not take the plan's API claims on trust — I read the actual `core::explore`
surface and the current `cli/src/mcp.rs` / `cli/src/main.rs` before ruling.

| Plan assumption | Verified against | Result |
|---|---|---|
| `grove_core::explore::{health_probe, run_explore, ExploreConfig, ExploreError, OpenAiCompatClient}` importable from crate root | `core/src/explore/mod.rs` re-exports | ✓ all re-exported (agent/client/config) |
| `run_explore(&question, root, cfg, &client)` | `agent.rs#run_explore@112` — `(question:&str, root:&Path, cfg:&ExploreConfig, client:&dyn ChatClient) -> Result<ExploreAnswer, ExploreError>` | ✓ arg order + `&OpenAiCompatClient`→`&dyn ChatClient` coercion valid |
| `ExploreError::ProviderDown { url, detail }` + `ExploreError::Client(msg)` exhaustive | `agent.rs` enum (2 variants only) | ✓ match is exhaustive |
| `answer.text` | `ExploreAnswer.text: String` | ✓ |
| `health_probe(&cfg) -> Result<(), _>`; unhealthy = unreachable **or** model-missing (AC-2) | `client.rs#health_probe@562` returns `HealthError::{Unreachable, ModelMissing}` | ✓ covers both |
| `explore_instructions(cfg)` uses `cfg.model` / `cfg.base_url` | `config.rs` `ExploreConfig` fields | ✓ |
| `ExploreConfig::config_path(root)` / `::load(root)`, `OpenAiCompatClient::new(cfg)` | config.rs:156/165, client.rs:485 | ✓ |
| `serve()`/`handle()` are the described refactor points; unit tests call `handle()` directly | `mcp.rs:27`, `mcp.rs:74`, tests mod | ✓ scope correct |

## Category assessment

- **Correctness** — Surface-once-at-startup (D3) is the right shape; every AC maps
  to a concrete change. `determine_surface` precedence (force_standard wins,
  then force_explore/config presence, then health gate) is coherent.
- **Security** — No new attack surface; explore tool schema is plain-object
  (no top-level anyOf/oneOf per the MCP checklist), question:string required.
  stdout/stderr discipline (protocol on stdout, diagnostics on stderr) preserved.
- **Architecture** — Keeps `mcp.rs` thin (dispatch/format) and delegates execution
  to `core::explore`, matching the task directive. Unhealthy path reuses existing
  code verbatim — no re-implementation, so AC-5 byte-identical default is credible.
- **Conventions** — clap struct-variant migration with a defaulted `path` positional
  is backward compatible (`grove serve` no-args still works).
- **Business rules** — n/a beyond ACs.
- **Testing** — See advisory notes; the one new test (AC-6 fallback) is deterministic
  (port 1, no provider), which is correct.

## Advisory notes (non-blocking)

1. **Thin test coverage for the explore surface.** The plan adds only the AC-6
   fallback test. The missing-`question`-argument → `isError` branch of
   `call_explore_tool` is deterministically unit-testable **without** a provider —
   consider adding it so AC-3's "bad args → isError, not JSON-RPC error" has a
   regression guard. (The healthy/mid-session provider paths genuinely need a live
   provider and `OpenAiCompatClient` is concrete/not injected, so leaving those to
   the core-level `connection_error_maps_to_provider_down` test is acceptable.)
   Also consider a trivial unit assertion that `explore_tool_spec()` has no
   top-level `anyOf`/`oneOf` (mirrors `every_tool_schema_is_client_registerable`
   for the standard surface) to lock AC-2's schema shape.
2. **AC-7 "no fastcontext string"** is a stated acceptance criterion but is not
   called out in the plan's approach/testing. Confirm during implementation
   (a grep in the review-code phase will suffice).
3. **`--explore` with no `.grove/explore.json`.** `determine_surface` computes
   `is_explore=true`, then `ExploreConfig::load` fails → fallback to Standard with
   an eprintln. Defensible (cannot explore without provider config), but ensure the
   stderr note distinguishes "load failed" from "provider unhealthy" so the user
   knows to run `grove config`/init rather than debug their endpoint.
4. **`--explore --standard` together** resolves to Standard (force_standard short-
   circuits). Fine as a precedence rule; no error needed.
