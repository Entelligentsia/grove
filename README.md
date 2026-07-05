# grove — structural sight for coding agents

grove gives coding agents **structural, byte-precise, token-cheap access to a
codebase** via tree-sitter — instead of reading whole files. One engine, seven
tools, **two faces** (a human CLI `grove <verb>` and an MCP server `grove serve`),
with grammars loaded **at runtime from a WASM registry**, so adding a language
needs no recompile and no toolchain on the consumer.

![grove in action — install + an agent answering a question with grove, no grep, no whole-file reads](docs/assets/grove_demo.gif)

*(asciinema cast: [`docs/assets/grove_demo.cast`](docs/assets/grove_demo.cast) —
play it interactively with `asciinema play docs/assets/grove_demo.cast`.)*

## Why grove

Agents burn tokens and round-trips `grep`-ing and `read`-ing whole files to
answer "where is this defined / what does it do / who calls it." grove replaces
that with **one symbol at a time, by exact bytes**, behind a stable id the agent
passes between turns.

- **Token-cheap** — `outline` a 1700-line file as a skeleton; `source` one
  symbol's body, not the whole file. A `map` call returns a directory's
  definitions + references in one shot.
- **Byte-precise & stable** — every result carries a `symbol-id`
  (`<lang>:<relpath>#<name>@<line>`, 1-based) you pass forward across turns.
- **One engine, two faces** — the same Rust binary drives the CLI and an MCP
  server, so a human and an agent see the same thing.
- **Runtime grammars** — all 27 official tree-sitter grammars load from a hosted
  WASM registry; new languages are a registry entry, not a recompile.

**Not an LSP.** grove is a syntactic, tree-sitter-powered shell for agents — not
a semantic language server. It speaks MCP (not LSP), parses (doesn't analyze),
and locates (doesn't refactor): no type inference, completion, rename, or
type-resolved go-to-def. It's the cheap syntactic layer *beneath* where an LSP's
semantics begin — complementary, not competitive. Full reasoning:
[Is grove an LSP?](docs/faq.md).

**27 languages out of the box** — one binary, grammars loaded at runtime from
the [hosted WASM registry](docs/languages.md):

<table>
<tr><td><img src="docs/assets/langs/bash.svg" width="18" height="18" alt="Bash" valign="middle">&nbsp;<b>Bash</b></td><td><img src="docs/assets/langs/c.svg" width="18" height="18" alt="C" valign="middle">&nbsp;<b>C</b></td><td><img src="docs/assets/langs/cpp.svg" width="18" height="18" alt="C++" valign="middle">&nbsp;<b>C++</b></td><td><img src="docs/assets/langs/c_sharp.svg" width="18" height="18" alt="C#" valign="middle">&nbsp;<b>C#</b></td><td><img src="docs/assets/langs/go.svg" width="18" height="18" alt="Go" valign="middle">&nbsp;<b>Go</b></td><td><img src="docs/assets/langs/java.svg" width="18" height="18" alt="Java" valign="middle">&nbsp;<b>Java</b></td><td><img src="docs/assets/langs/javascript.svg" width="18" height="18" alt="JavaScript" valign="middle">&nbsp;<b>JavaScript</b></td><td><img src="docs/assets/langs/julia.svg" width="18" height="18" alt="Julia" valign="middle">&nbsp;<b>Julia</b></td></tr>
<tr><td><img src="docs/assets/langs/php.svg" width="18" height="18" alt="PHP" valign="middle">&nbsp;<b>PHP</b></td><td><img src="docs/assets/langs/python.svg" width="18" height="18" alt="Python" valign="middle">&nbsp;<b>Python</b></td><td><img src="docs/assets/langs/ruby.svg" width="18" height="18" alt="Ruby" valign="middle">&nbsp;<b>Ruby</b></td><td><img src="docs/assets/langs/rust.svg" width="18" height="18" alt="Rust" valign="middle">&nbsp;<b>Rust</b></td><td><img src="docs/assets/langs/scala.svg" width="18" height="18" alt="Scala" valign="middle">&nbsp;<b>Scala</b></td><td><img src="docs/assets/langs/typescript.svg" width="18" height="18" alt="TypeScript" valign="middle">&nbsp;<b>TypeScript</b></td><td><img src="docs/assets/langs/typescript.svg" width="18" height="18" alt="TSX" valign="middle">&nbsp;<b>TSX</b></td><td><kbd>Agda</kbd><sup>2</sup></td></tr>
<tr><td><img src="docs/assets/langs/css.svg" width="18" height="18" alt="CSS" valign="middle">&nbsp;<b>CSS</b><sup>2</sup></td><td><kbd>Embedded&nbsp;Template</kbd><sup>2</sup></td><td><img src="docs/assets/langs/haskell.svg" width="18" height="18" alt="Haskell" valign="middle">&nbsp;<b>Haskell</b><sup>2</sup></td><td><img src="docs/assets/langs/html.svg" width="18" height="18" alt="HTML" valign="middle">&nbsp;<b>HTML</b><sup>2</sup></td><td><kbd>JSDoc</kbd><sup>2</sup></td><td><img src="docs/assets/langs/json.svg" width="18" height="18" alt="JSON" valign="middle">&nbsp;<b>JSON</b><sup>2</sup></td><td><img src="docs/assets/langs/ocaml.svg" width="18" height="18" alt="OCaml" valign="middle">&nbsp;<b>OCaml</b><sup>2</sup></td><td><img src="docs/assets/langs/ocaml.svg" width="18" height="18" alt="OCaml Interface" valign="middle">&nbsp;<b>OCaml&nbsp;Interface</b><sup>2</sup></td></tr>
<tr><td><kbd>CodeQL</kbd><sup>2</sup></td><td><kbd>Regex</kbd><sup>2</sup></td><td><kbd>Verilog</kbd><sup>2</sup></td><td></td><td></td><td></td><td></td><td></td></tr>
</table>

<sup>2</sup> minimal profile — core tools only (`callers`/`definition` degrade);
full profile = all tools. `<kbd>` = no official logo. Profiles are data, not
compiled in. See [Languages & grammars](docs/languages.md).

See [`VISION.md`](VISION.md) for the product vision.

## Quick start

```bash
# 1. install (one line — detects platform, verifies sha256)
curl -fsSL https://raw.githubusercontent.com/Entelligentsia/grove/main/install.sh | sh

# 2. wire it into a project (in the project root)
grove init
```

`grove init` detects the project's languages, auto-fetches their grammars, and
writes `.mcp.json` (the tools exist) + a `CLAUDE.md` steering directive (the
agent reaches for grove instead of grep) + `grove.lock`. That's it — your agent
now has structural sight. Other install channels (Homebrew, npm, cargo, agent
skill) and `--as mcp|skill|both|mcp-llm` are in **[Install](docs/install.md)** and
**[Setup](docs/setup.md)**.

> As an **agent skill** (Claude Code, Cursor, Codex, Cline, …):
> `npx skills add Entelligentsia/grove` — the skill self-installs the binary on
> first use if it's missing. See [Setup](docs/setup.md).

## Evaluated on real codebases

grove is measured in **[is-grep-enough](https://github.com/Entelligentsia/is-grep-enough)** —
a fair, blind-judged comparison of three navigation regimes (text `baseline`,
structural `grove`, semantic `lsp`) given the **same agent the same prompt**
across **50 tasks**: 10 large, popular, grammar-backed repos × 5 rungs of
climbing complexity (locate a symbol → trace flow → recover an architecture).
Same substrate, one variable — the navigation capability. Explore every run in
the **[live dashboard](https://entelligentsia.github.io/is-grep-enough/)**.

[![Same answer, far fewer tokens — across 5 rungs of task complexity grove ties on answer quality and runs about 2x leaner on context, 2.8x leaner on the hardest tasks](docs/assets/grove_context_curve.svg)](https://entelligentsia.github.io/is-grep-enough/)

The result is a **curve, not a verdict**:

- **Answer quality ties — and on the hardest traces grove is the more reliable
  one.** Grounding ~0.97 and completeness ~0.99 across all three arms; grep is
  enough to be *correct* most of the time. But where text search drifts on a
  dense trace — the C++ L5 architecture trace drops baseline grounding to
  **0.80** — grove holds **0.97**, because it cites the real syntax tree instead
  of guessing line numbers.
- **grove runs leaner on context, and the lead widens with complexity.** ~395K
  mean tokens vs 567K (lsp) and 780K (baseline) overall — roughly half the
  text-search context. By the hardest rung (architecture / binding-spine),
  baseline pushes ~1.5M tokens against grove's ~534K — **2.8× leaner** (on the
  TypeScript L5 trace, 2.43M → 570K, **4.3× leaner**) — while grove stays
  tightest on quality.
- **Reported honestly, including where grove doesn't win.** On trivial
  *locate-one-symbol* tasks (L1), grove's fixed structural-call overhead means
  it isn't the cheapest — plain text search is. grove pays off on the navigation
  that actually costs tokens: tracing flow, mapping a subsystem, recovering an
  architecture.

Token throughput isn't the billed bill — much of baseline's volume is cheap
cache reads — but it is what drives context-window pressure and latency. Full
methodology, per-repo data, blind judgements, and every raw transcript:
**[is-grep-enough](https://github.com/Entelligentsia/is-grep-enough)**
· **[live dashboard](https://entelligentsia.github.io/is-grep-enough/)**.

## The tools

| | Command | What it returns |
|---|---|---|
| outline | `grove outline <file>` | a file's definition skeleton (kind · name · parent · signature · id) |
| symbols | `grove symbols <dir> --name <n>` | repo-wide symbol search — `--name` is **exact**, `--name-contains` for substring |
| source | `grove source <id>` | one symbol's full source — no whole-file read |
| check | `grove check <file>` | ERROR / MISSING nodes — post-edit syntax check (exit 1 if any) |
| callers | `grove callers <name> -d <dir>` | call sites of a symbol, each with its enclosing function |
| map | `grove map <dir>` | directory dependency graph: definitions + outgoing references, no bodies |
| definition | `grove definition <name>` / `--at <f:l:c>` | go-to-def, by name or from a usage position |

Add `--json` to any command for the agent-facing shape. Full reference + examples:
**[Tools](docs/tools.md)**.

## Delegated local-LLM mode (`mcp__grove__explore`) · ⚠️ Experimental

> **Experimental / opt-in — unreleased.** The default grove (the CLI and the
> standard 7-tool `grove serve`) is unaffected. This mode's behaviour, its config
> (`.grove/explore.json`), and the tool contract may change without notice.

**What it is**: `mcp__grove__explore` is a single MCP tool the outer coding agent
calls with **one narrow "where is X" question**. Grove's inner Rust explorer
agent drives a short, bounded tool-calling loop locally — against your configured
local / OpenAI-compatible LLM (Ollama, llama.cpp) — and returns a short
explanation plus **validated `file:line` citations**. It is a *locator* (it finds
WHERE relevant code lives), not a full-analysis oracle: ask a few targeted
questions and synthesize the results yourself. The outer agent never sees the
inner tool calls — and never spends its own context on them.

```
   outer coding agent (Claude / Cursor / …)
      │  explore("where is session-cookie signing?")     ▲  explanation +
      │  — one narrow locator question —                 │  file:line citations
      ▼                                                  │  (+ live progress ticks)
 ┌────────────────────  grove serve --explore  ──────────┴──────────────────────┐
 │  startup health probe (/models):                                             │
 │     healthy   → surface ONLY  mcp__grove__explore                            │
 │     unhealthy → fall back to the 7 structural tools   (never a dead server)  │
 │                                                                              │
 │  inner explorer  (core::explore) — a port of the proven delegation agent:    │
 │     bounded loop  ≤ 6 turns · forced final answer at the cap · no byte budget│
 │                                                                              │
 │         local LLM  ⇄  Grove · Read · Glob · Grep                             │
 │      (Ollama / llama.cpp,     Grove → grove binary,  Grep/Glob → ripgrep,    │
 │       OpenAI-compatible)      Read in-process                                │
 │                                                                              │
 │     arms:  standard (merit) · balanced (plan-first) · strict (grove-first)   │
 │     result: <final_answer> citations, each validated against the filesystem  │
 └──────────────────────────────────────────────────────────────────────────────┘
```

**Setup**:
```
grove init --as mcp-llm   # interactive setup TUI (requires TTY)
grove config              # revisit / change settings at any time
```

**Three steering modes** (trade-off in one line each):

| Mode | Trade-off |
|---|---|
| `standard` | inner model picks tools naturally — lowest overhead, works well with capable models |
| `balanced` | two-phase plan → execute — best grounding and lowest hallucination rate, highest wall-clock |
| `strict` | grove-first steering prompts — cost/quality sweet spot for smaller models |

Change the active mode at any time with `grove config`.

**Health semantics**:
- Startup: `grove serve --explore` probes the configured provider (`/models`).
  - **Healthy** → expose `mcp__grove__explore` only.
  - **Unhealthy at startup** → transparent fallback: the 7 structural tools
    (`outline`, `symbols`, `source`, `check`, `callers`, `map`, `definition`)
    are surfaced instead so the outer agent is never left tool-less.
- Mid-session loss → `mcp__grove__explore` returns a recoverable `isError`
  response with a restart hint; the outer agent can retry or degrade gracefully.

**Debugging — see the inner conversation.** Turn on **Tap** (a `tap` flag in
`.grove/explore.json`, toggled in `grove config` — or just run `grove tap`, which
flips it on for you). `grove serve --explore` then records each session to a
per-session JSONL trace under `.grove/traces/`: a header with the calling agent's
identity, model and mode, then a `call_start` / `turn` / `call_end` stream per
`explore` call with **token usage and wall time**.

Run **`grove tap`** to browse them in a full-screen TUI — drill session → call →
turn: the session list shows agent, model, call count, total tokens and a live
marker; opening a call shows its metrics and each turn's request/response. It
refreshes live, so you can watch a session as it runs. Retention keeps the last
`trace_retain` sessions (default 50). `grove tap --no-enable` opens the browser
without changing the config.

## As a library — `grove-core`

The same engine ships as a standalone crate, **`grove-core`**, so you can embed
grove's structural queries directly in Rust — no subprocess, no CLI. The `grove`
binary is a thin `clap` + MCP shell over it. The crate is **`clap`-free**;
grammars still load at runtime from the WASM registry, so nothing is compiled in.

On crates.io as **`grove-cst`** — CST for the *concrete syntax trees* tree-sitter
builds (`grove-core` is taken by an unrelated crate). Alias it so imports stay
`use grove_core::…`:

```toml
# Cargo.toml
[dependencies]
grove_core = { package = "grove-cst", version = "0.1" }
```

```rust
use std::path::Path;
use grove_core::{init, ops};

fn main() -> anyhow::Result<()> {
    let project = Path::new(".");

    // 1. Provision grammars for this project's languages — fetches any missing
    //    grammar into the OS cache and pins grove.lock. Run once.
    for action in init::provision_project(project, false)? {
        println!("provisioned: {action}");
    }

    // 2. Query — grammars resolve from the cache. Every definition under `src/`,
    //    gitignore-aware, as typed results.
    for s in ops::symbols(&project.join("src"), None, None, false, false)? {
        println!("{} {} — {}:{}", s.kind, s.name, s.file, s.line);
    }
    Ok(())
}
```

(Offline? Set `GROVE_REGISTRY=<dir>` to resolve grammars from a pinned registry
and skip the fetch — see [`core/README.md`](core/README.md).)

The consumer surface is the [`ops`](core/src/lib.rs) module — the same seven
tools (`outline`, `symbols`, `source`, `check`, `callers`, `map`, `definition`),
returning typed `Symbol` / `Defect` / `CallSite` / `FileMap` values (re-exported
at the crate root). `init::provision_project` is the grammar-provisioning entry
point behind `grove init`. Crate overview and full API surface:
[`core/README.md`](core/README.md) · [`core/src/lib.rs`](core/src/lib.rs).

## Documentation

- **[Install](docs/install.md)** — curl/Homebrew/npm/cargo, build from source, the agent skill
- **[Setup](docs/setup.md)** — `grove init`, `--as mcp|skill|both|mcp-llm`, what it writes, offline/dry-run
- **[Languages & grammars](docs/languages.md)** — the WASM registry, `fetch`/`lock`, where grammars live, profiles
- **[Tools](docs/tools.md)** — the seven tools, `--json`, `symbol-id`, examples
- **[MCP server](docs/mcp.md)** — `grove serve`, `.mcp.json`, steering, error model
- **Library** — [`grove-core`](core/README.md): embed the engine in Rust (no CLI, no subprocess)
- **[Roadmap & repo layout](docs/roadmap.md)** — what's not done yet, source map
- **[FAQ](docs/faq.md)** — *Is grove an LSP?* and other positioning questions
- [`VISION.md`](VISION.md) — product vision · [`CHANGELOG.md`](CHANGELOG.md) — releases
- Eval: [`Entelligentsia/is-grep-enough`](https://github.com/Entelligentsia/is-grep-enough) — text vs structural vs semantic navigation, same agent, 50 blind-judged tasks · [live dashboard](https://entelligentsia.github.io/is-grep-enough/)
- Registry: [`Entelligentsia/grove-registry`](https://github.com/Entelligentsia/grove-registry) · Homebrew tap: [`Entelligentsia/homebrew-grove`](https://github.com/Entelligentsia/homebrew-grove)

## Status

Pre-1.0. `callers`/`definition` are name-based (no receiver-type resolution); 12
languages ship a minimal profile (core tools only); no incremental reparse yet.
Details and the rest of the roadmap: **[Roadmap](docs/roadmap.md)**.