# Sprint Requirements — GROVE-S02

**Captured:** 2026-07-02
**Source:** sprint-intake interview (`/forge:new-sprint`)

---

## Background & Evidence

This sprint productizes the **local-delegation sidebench** findings
(`../code-analyzer-testbench/studies/fastcontext-sidebench/`): replacing Claude
driving grove directly with Claude *delegating all exploration* to a free local
model (qwen3.5:4b via Ollama, driving grove internally through a single
`explore` MCP tool) cut **total metered context ~16–18× (mean) / ~5.8× (median)**
versus the text baseline, at grounding parity (0.93–0.95 vs 0.94). Three inner
steering variants were measured:

| Sidebench arm | Behavior | Result summary |
|---|---|---|
| **merit** | grep-natural — inner model chooses tools freely | 100% completeness, slowest scaling on hard cells |
| **coerce** | grove-forced — strong grove-first steering | cost/quality sweet spot: lowest wall, −8.6% ctx vs merit |
| **plan-first** | harness-enforced two-phase recon→plan→execute (recon-once cached per session) | best grounding (0.95), highest wall |

The sidebench's Python explorer was an **experimental agent only**. The
production implementation in this sprint is **pure Rust inside grove** — and
the term "fastcontext" must not appear anywhere in the grove implementation
(code, identifiers, config keys, tool names, docs shipped with the binary).

The current grove MCP surface lives in `cli/src/mcp.rs` (post GROVE-S01
workspace split: `core/` = grove-core library, `cli/` = grove binary).
`grove init` today supports `--as mcp|skill|both` (`cli/src/init.rs`,
`Target` enum).

## Decisions (resolved from intake open questions)

| # | Question | Decision |
|---|---|---|
| D1 | Explorer implementation | **Pure Rust**, native in grove. The Python sidebench agent was experimental; nothing is vendored or wrapped. |
| D2 | Provider scope | **OpenAI-compatible is the target transport.** Ollama is the **default** provider; llama.cpp is also supported. |
| D3 | Health-gate semantics | **Startup probe decides the surface.** If the provider is healthy → surface explore-only (`mcp__grove__explore`). If it is **unhealthy at startup → fall back to the standard `--mcp` surface** (the 7 structural tools), so grove stays useful without the local LLM. Never a dead server, never a half-alive erroring state. Mid-session provider loss returns a recoverable `isError` on the `explore` call (see item 7). |
| D4 | TUI depth | **Solid (full-screen) TUI.** Additionally, a `grove config` verb surfaces the same TUI at any time to change existing config. |
| D5 | Tool surface | In mcp-llm mode **only `mcp__grove__explore` surfaces** — the standard 7 structural tools do NOT. `grove serve` therefore gains a **mode**. No "fastcontext" naming anywhere. |
| D6 | Success measurement | **Functional correctness only.** Benchmarking is a post-implementation activity (grove-testbench). |

## Goals

1. A user can run `grove init --as mcp-llm` in a project and, after a
   full-screen TUI setup flow, get a working `mcp__grove__explore` tool that
   delegates local file/code exploration to a locally configured LLM — the
   *only* tool the outer (metered) agent sees from grove in this mode.
2. Exploration behavior is selectable via a mode config with three levels:
   **standard** (merit), **balanced** (plan-execute), **aggressive**
   (grove-first strong steering).
3. Harness steering files (`CLAUDE.md`, `AGENTS.md`) are updated by init so the
   outer agent routes exploration calls through `__explore`.
4. A `grove serve` started in mcp-llm mode probes the provider at startup: if
   healthy it surfaces explore-only; if unhealthy it **falls back to the
   standard 7-tool structural surface** rather than dying, so grove is always
   useful.
5. `grove config` re-opens the setup TUI at any time to modify existing
   configuration.

## In Scope

### 1. New init mode: `grove init --as mcp-llm` [must-have]
Extends the existing `--as mcp|skill|both` target set with `mcp-llm`.

**Acceptance criteria:**
- `grove init --as mcp-llm` is accepted by the CLI and documented in `--help`.
- The mode provisions grammars (as existing modes do) *and* writes the
  mcp-llm configuration produced by the setup TUI (provider, URL, model,
  allowed tools, mode) to a project-local config file.
- Existing `--as mcp|skill|both` behavior is unchanged (regression: current
  `tests/cli.rs` init assertions still pass).

### 2. Pure-Rust inner explorer engine [must-have]
The agent loop behind `__explore`: an OpenAI-compatible chat client plus a
tool-execution loop, implemented in Rust in `core/` (no Python, no vendored
code, no "fastcontext" naming).

**Acceptance criteria:**
- Speaks the **OpenAI-compatible** chat-completions protocol with tool calls;
  works against **Ollama** (default) and **llama.cpp server** endpoints.
- The inner loop can invoke grove structural operations and the allowlisted
  bash tools, and returns a synthesized exploration answer as the
  `__explore` tool result.
- Tool gating is enforced at schema *and* execution level (small local models
  hallucinate closed tools — sidebench-proven; prompt-only gating scored 0/4).
- No identifier, config key, tool name, or shipped doc contains "fastcontext".

### 3. Setup TUI + `grove config` [must-have]
A solid full-screen TUI, launched by `init --as mcp-llm` and re-launchable at
any time via a new `grove config` verb, that collects and edits:

- **Local LLM endpoint**: URL of the local model server.
- **Provider**: Ollama (default) or llama.cpp — both over OpenAI-compat.
- **Model**: model identifier to use for exploration.
- **Setup option / mode**: standard, balanced, or aggressive (see item 4).
- **Allowed bash tools**: multi-select of shell tools the inner explorer may
  invoke — e.g. `grove`, `find`, `grep`, `ripgrep` (extensible list).

**Acceptance criteria:**
- Running `init --as mcp-llm` on a TTY presents the TUI; selections persist to
  the mcp-llm config file.
- `grove config` re-opens the TUI pre-populated with the existing config, and
  saved changes take effect on the next `grove serve` start.
- The allowed-tools selection is enforced at runtime: the inner explorer
  cannot invoke a shell tool not on the allowlist (verify with a config that
  omits e.g. `find`).
- A non-TTY invocation (CI/pipe) does not hang: it either accepts equivalent
  flags or fails fast with a descriptive message.

### 4. Mode config: standard / balanced / aggressive [must-have]
Three named exploration modes mapping to the sidebench arms:

| Mode | Sidebench arm | Inner behavior |
|---|---|---|
| `standard` | merit | inner model chooses tools naturally (grep-natural) |
| `balanced` | plan-first | two-phase plan→execute (recon with structure verbs, committed plan, then free execution) |
| `aggressive` | coerce | grove-first strong steering — structural tools forced before text search |

**Acceptance criteria:**
- Mode is selectable in the TUI and stored in config.
- Each mode observably changes the inner explorer's steering (distinct system
  prompts / tool-gating per mode, verifiable in a transcript or debug log).
- `balanced` enforces plan-then-execute at the harness level (tool gating),
  not by prompt request alone.

### 5. `grove serve` mode + exclusive tool surface [must-have]
`grove serve` gains a mode: in mcp-llm mode it surfaces **only**
`mcp__grove__explore`; the standard 7 structural tools are not listed.

**Acceptance criteria:**
- With an mcp-llm config present (or the mode explicitly selected) **and the
  provider healthy at startup**, `tools/list` returns exactly one tool:
  `explore` (surfacing to clients as `mcp__grove__explore`).
- With an mcp-llm config present but the provider **unhealthy at startup**,
  `tools/list` returns the standard 7 structural tools (the `--mcp` surface) —
  see item 7.
- Without mcp-llm mode, `grove serve` behaves exactly as today (7 structural
  tools; regression: existing MCP smoke test unchanged).
- The structural operations remain available *internally* to the inner
  explorer even though they are not surfaced to the outer agent in explore mode.

### 6. Harness steering: CLAUDE.md + AGENTS.md routing [must-have]
Init writes/updates steering so the outer agent routes exploration through
`__explore`.

**Acceptance criteria:**
- `init --as mcp-llm` adds a grove-managed steering section to `CLAUDE.md`
  **and** `AGENTS.md` directing the agent to use `__explore` for file/code
  exploration instead of built-in search/read tools.
- Steering blocks are idempotent (re-running init does not duplicate them),
  consistent with the existing `write_claude_md` sentinel approach.
- `.mcp.json` (or equivalent harness registration) is written so the project's
  grove server starts in mcp-llm mode.

### 7. Health-gated startup with structural fallback [must-have]
The startup probe decides which surface `grove serve` presents; the server is
always useful.

**Acceptance criteria:**
- At `grove serve` startup in mcp-llm mode, grove probes the configured
  provider. **Healthy** → surface explore-only (`__explore`). **Unhealthy**
  (unreachable or model-missing) → **fall back to the standard 7-tool
  structural surface** (identical to `--as mcp`), with a descriptive stderr
  note that it fell back and why. The server always comes up and always
  answers `initialize` — never a dead server.
- A provider **connection error mid-session** (was healthy at startup, so only
  `__explore` was surfaced) returns a recoverable `isError: true` result on
  the `explore` call with an actionable message (provider down; check the
  endpoint / run `grove config` / restart). The server does not crash.
- The health probe and its outcome (healthy → explore, unhealthy → fallback)
  are observable (stderr log line or status output).

## Out of Scope

- Changes to the seven existing structural tools' contracts
  (`outline`/`symbols`/`source`/`check`/`callers`/`map`/`definition`).
- Any Python: the sidebench explorer is experimental and is not vendored,
  wrapped, or shipped.
- Cloud/metered providers (Anthropic, OpenAI cloud) — local endpoints only.
- Fine-tuning or shipping any local model; the user brings their own endpoint.
- Benchmarking / token-reduction measurement — post-implementation activity
  (grove-testbench), not an acceptance gate for this sprint (D6).

## Nice-to-Have *(attempt if must-haves complete)*

- Model auto-discovery in the TUI (e.g. populate the model list from Ollama's
  `/api/tags`).
- Recon-plan caching for `balanced` mode (recon-once per session, as validated
  in the sidebench: −27% ctx / −35% wall vs recon-every-call).

## Constraints

- **Pure Rust, single binary:** the inner explorer, TUI, and OpenAI-compat
  client are all in-binary; distribution via cargo/brew/npm prebuilts is
  unaffected.
- **Naming:** the term "fastcontext" must not appear anywhere in the grove
  implementation.
- **Workspace split:** engine logic (explorer loop, provider client, config
  model) belongs in `core/` (grove-core, clap-free); `cli/` stays thin
  (`main.rs`/`mcp.rs`/TUI presentation only).
- **Toolchain:** cargo 1.87; crates.io deps only (no tree-sitter workspace path
  deps). `cargo build` warning-clean; `cargo clippy -- -D warnings` clean.
- **Conventions:** conventional commits, files end with newline, no
  `Co-Authored-By`.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Native inner agent loop (LLM client + tool loop + plan phases) is a substantial new subsystem | High | Keep the loop minimal (chat + tool-calls only); mode differences are prompt/gating data, not code paths |
| Small local models hallucinate closed tools / ignore prompt-only steering | High | Enforce gating at schema+execution level (sidebench-proven), not prompts |
| Mid-session provider loss leaves the outer agent seeing only `explore` (fallback-to-structural can't retroactively re-list without `tools/list_changed`) | Medium | Recoverable `isError` on the `explore` call with an actionable message; document that a restart re-runs the startup probe and picks up the structural fallback |
| Full-screen TUI dependency weight (ratatui + async runtime) in a lean binary | Medium | Audit binary size / compile-time impact; TUI code isolated in `cli/` |
| Quality regression vs direct-grove arm on hard repos (sidebench L5-tokio completeness 0.75) | Medium | Document mode trade-offs in steering; direct structural mode remains available via `--as mcp` |
| OpenAI-compat dialect drift between Ollama and llama.cpp (tool-call encoding differences) | Medium | Integration-test against both; keep provider quirks behind one client module |

## Carry-Over from GROVE-S01

| Item | Status | Notes |
|---|---|---|
| Workspace split (grove-core + cli) | Active | S02 builds on the split layout; `cli/src/mcp.rs` is the MCP surface, engine logic goes in `core/` |
