# Sprint Plan — GROVE-S02

**grove init --as mcp-llm — local-LLM delegated exploration (`mcp__grove__explore`)**

**Planned:** 2026-07-02
**Requirements:** [SPRINT_REQUIREMENTS.md](SPRINT_REQUIREMENTS.md)

---

## Approach

The sprint builds one new subsystem — a pure-Rust inner explorer — and threads it
through grove's existing seams. Decomposition follows the workspace split: the
**engine** (config model, provider client, agent loop) lands in `core/` as a new
`explore` module; the **presentation** (serve mode, TUI, init target, steering
writers) lands in `cli/`. Key structural choices:

1. **Bottom-up through core, then out through cli.** Config model first (T01) —
   it is the shared vocabulary every other task consumes. Then the
   OpenAI-compatible client (T02), then the agent loop that composes both (T03).
   Only then does the MCP surface change (T04), because `tools/list` gating,
   startup probe, and shutdown semantics all sit on top of the core engine.
2. **The client is a trait, not a singleton.** Small local models are exercised
   over an OpenAI-compat dialect that drifts between Ollama and llama.cpp; the
   agent loop takes a `ChatClient` trait object so the loop's gating logic
   (schema + execution level — the sidebench-proven requirement) is unit-testable
   without a live endpoint. Network paths are tested to the error-before-connect
   boundary, per existing project convention (no HTTP mocks).
3. **The health gate's negative path is the deterministic test.** "Startup probe
   fails → descriptive error, no explore surface" needs only an unreachable URL,
   so T04's acceptance rides on `tests/cli.rs` without any provider running.
4. **Mode differences are data, not code paths.** standard/aggressive differ
   only in system-prompt steering; balanced adds a harness-enforced two-phase
   gate (recon toolset + a plan-commit tool, then the full toolset). One loop,
   three configurations.
5. **TUI branches off early.** T05 needs only the config model (T01), so it runs
   parallel to the T02→T03→T04 engine chain. `grove init --as mcp-llm` (T06) is
   deliberately last-but-one: it is thin glue over TUI (T05) + serve mode (T04)
   + the existing `provision_project` and steering-writer seams from GROVE-S01.
6. **Naming ban is enforced, not requested.** T07 adds a repo-wide guard test
   that the string "fastcontext" appears nowhere in `core/`, `cli/`, or shipped
   docs (D1/D5).

Every task lands warning-clean (`cargo clippy -- -D warnings`) with
`cargo test --release --locked --workspace` green; default-mode `grove serve`
behavior is a frozen regression surface throughout.

## Tasks

| ID | Title | Est | Depends on |
|---|---|---|---|
| GROVE-S02-T01 | Explore config model + persistence (`core::explore::config`) | M | — |
| GROVE-S02-T02 | OpenAI-compatible chat client + health probe (`core::explore::client`) | M | T01 |
| GROVE-S02-T03 | Inner explorer agent loop with mode steering + tool gating (`core::explore`) | L | T02 |
| GROVE-S02-T04 | `grove serve` explore mode — exclusive tool surface + health-gated startup/shutdown | M | T03 |
| GROVE-S02-T05 | Full-screen setup TUI + `grove config` verb | L | T01 |
| GROVE-S02-T06 | `grove init --as mcp-llm` — target, harness steering (CLAUDE.md/AGENTS.md), .mcp.json | M | T04, T05 |
| GROVE-S02-T07 | Integration tests, naming guard, docs | M | T04, T06 |

## Dependency graph & critical path

```
T01 ──► T02 ──► T03 ──► T04 ──► T06 ──► T07
  │                       │      ▲
  └──► T05 ───────────────┼──────┘
                          └────────────► (T07 also gates on T04)
```

- **Critical path:** T01 → T02 → T03 → T04 → T06 → T07 (M+M+L+M+M+M).
- **T05** (TUI, L) branches off T01 and can run in parallel with T02–T04,
  rejoining at T06.
- **Execution mode:** sequential on the critical path; T05 is the one
  parallelizable branch.

## Per-task acceptance summary

- **T01** — `core/src/explore/config.rs` (module `core::explore` declared in
  `lib.rs`): `ExploreConfig { provider (Ollama|LlamaCpp), base_url, model, mode
  (Standard|Balanced|Aggressive), allowed_tools: Vec<String> }` with serde
  load/save/validate against a project-local file (`.grove/explore.json`);
  defaults = Ollama (`http://localhost:11434/v1`); descriptive errors on
  missing/invalid config; unit tests for round-trip, defaults, validation;
  clap-free core preserved.
- **T02** — `core::explore::client`: a `ChatClient` trait +
  `OpenAiCompatClient` (ureq/rustls) speaking chat-completions **with tool
  calls** (non-streaming), against Ollama and llama.cpp base URLs; a
  `health_probe()` that verifies endpoint + model availability, returning a
  typed error distinguishing unreachable / model-missing; provider quirks
  isolated in this one module; unit tests to the error-before-connect boundary
  (unreachable URL → typed error), request/response serde round-trip tests.
- **T03** — `core::explore::run_explore(question, &ExploreConfig, &dyn
  ChatClient) -> Result<ExploreAnswer>`: agent loop exposing the 7 structural
  ops (internal calls to `ops::…`) plus allowlisted shell tools (`grove`,
  `find`, `grep`, `rg`, … from config) to the inner model; gating enforced at
  **schema and execution** level (a tool absent from the advertised schema is
  also refused at dispatch); mode steering as data — `standard` (free choice),
  `aggressive` (grove-first steering prompt), `balanced` (two-phase: recon
  toolset + plan-commit tool, then full toolset — harness-enforced); loop
  bounded (max turns / max tokens); unit tests with a scripted fake
  `ChatClient` covering gating (hallucinated tool refused), mode toolset
  differences, and balanced-phase transition; runtime connection error
  surfaces as a typed error (T04 consumes it to shut down).
- **T04** — `grove serve` gains a mode: with an mcp-llm project config present
  (or explicit flag), `tools/list` returns **exactly one** tool `explore`
  (client-side `mcp__grove__explore`); startup health probe failure → server
  exits with a descriptive stderr message and never surfaces the tool; a
  provider connection error mid-session → descriptive shutdown (typed error
  from T03), no half-alive state; **default-mode `grove serve` byte-identical**
  (existing `tests/cli.rs` MCP cases + smoke test pass unchanged); new
  integration case: explore mode with unreachable provider fails fast.
- **T05** — full-screen TUI (ratatui, `cli/`-side only) collecting provider,
  base URL, model, mode, and allowed-tools multi-select; new `grove config`
  verb re-opens it pre-populated from the existing config, saving via T01
  persistence (effective on next `grove serve`); non-TTY invocation fails fast
  with a descriptive message (no hang); [nice-to-have] model list
  auto-discovery from the provider; binary-size / compile-impact noted in PR.
- **T06** — `Target::McpLlm` (`--as mcp-llm`) in `cli/src/init.rs`: runs
  `provision_project` (grammars + lock), launches the T05 TUI to produce
  `.grove/explore.json`, writes `.mcp.json` registering grove serve in explore
  mode, and writes **idempotent** steering sections to `CLAUDE.md` **and**
  `AGENTS.md` (sentinel-block pattern from the existing `write_claude_md`)
  routing exploration through `__explore`; existing `--as mcp|skill|both|grammars`
  behavior unchanged (existing init tests pass); `--dry-run` supported.
- **T07** — end-to-end hardening: `tests/cli.rs` coverage for `grove config`
  non-TTY, init idempotency (re-run duplicates nothing), allowlist enforcement
  (config omitting `find` → inner dispatch refuses it); repo-wide naming guard
  test (no "fastcontext" in `core/`, `cli/`, `README.md`, `skills/`); README +
  CHANGELOG sections for mcp-llm mode, `grove config`, and the
  health/shutdown semantics; stack-checklist review (MCP schema plainness,
  stdio hygiene, exit codes).

## Risks carried into execution

| Risk | Owning task | Handling |
|---|---|---|
| Agent-loop scope blow-up | T03 | Loop is minimal chat+tool-calls; modes are data; bounded turns |
| Ollama vs llama.cpp tool-call dialect drift | T02 | One client module owns quirks; both endpoints in acceptance |
| ratatui dependency weight | T05 | cli-only dep; measure build/binary impact in the task |
| Mid-session shutdown surprises MCP clients | T04 | Descriptive stderr message; documented in T07 |
| Small-model tool hallucination | T03 | Schema **and** execution gating; fake-client unit test proves refusal |

## Estimation basis

No token-cost reporting was available at planning time (`/cost` probe
unavailable); estimates are complexity-based (S/M/L) against GROVE-S01 actuals
(workspace split tasks, M–L each).
