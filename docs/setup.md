# Setup — `grove init`

One command makes a project one where the agent *uses* grove:

```bash
grove init           # in your project root
```

It detects the project's languages from the hosted catalog (so it sees a
language even before its grammar is installed), **auto-fetches** the grammars the
project needs, then writes three things (idempotently, preserving anything
already there):

- **`.mcp.json`** — registers the grove MCP server (*availability* — the tools exist).
- **`CLAUDE.md`** — a steering directive in a marked section (*adoption* — the agent
  reaches for grove instead of grep/whole-file reads; see [VISION §6.4.1](../VISION.md)).
- **`grove.lock`** — pins the detected grammars' version + wasm sha256.

`grove init --dry-run` detects without writing or fetching. Re-running only
updates grove's own pieces. Offline, it falls back to detecting from grammars
already in the cache.

## MCP, skill, or both — `grove init --as`

grove has one engine behind three faces: the CLI, the MCP server, and a
**cross-harness skill**. `--as` selects which integration `init` wires up
(grammar provisioning + `grove.lock` happens for every target):

```bash
grove init --as mcp      # default — .mcp.json + CLAUDE.md + grove.lock
grove init --as skill    # grammars + grove.lock only; install the skill separately
grove init --as both     # MCP wiring and grammars, for skill + MCP side by side
grove init --as mcp-llm  # both servers: grove (structural) + grove-explore (locator) + CLAUDE.md + AGENTS.md
```

## Explore-mode — `grove init --as mcp-llm`

> **Opt-in, and a second server, not a mode.** The `.grove/config.json` config
> format and the `explore` tool contract are covered by semantic versioning as
> of 0.3.0. The standard `--as mcp|skill|both` targets and the always-structural
> 7-tool `grove serve` are unaffected either way — `grove-explore` is a
> separate binary with its own MCP server identity that you register
> *alongside* `grove`, never instead of it.

`--as mcp-llm` registers **both servers** in `.mcp.json`: `grove` (the 7-tool
structural surface, unconditionally) and `grove-explore` (a single `explore`
tool backed by a local LLM, configured via `.grove/config.json`'s `explore`
section). The outer agent asks `mcp__grove-explore__explore` narrow *where-is*
questions and uses `mcp__grove__source` / `mcp__grove__map` directly on the
cited `file:line` for precision work — the two surfaces compose. If the
explore provider is unhealthy at startup, `grove-explore serve` fails to start
with an actionable error; there is no fallback that swaps in a different tool
surface, and the `grove` structural server is unaffected either way.

What it writes:

- **`.mcp.json`** — registers **two** MCP servers: `grove` (`grove serve`) and
  `grove-explore` (`grove-explore serve`).
- **`CLAUDE.md`** — dual-surface steering block naming both servers
  (`mcp__grove__*` and `mcp__grove-explore__explore`) and the recommended
  delegation flow: narrow `explore` question → `source`/`map` on the cited
  `file:line` → synthesize.
- **`AGENTS.md`** — the same dual-surface steering, harness-neutral, for
  non-Claude harnesses (Codex, Cline, etc.).

**First-run TUI**: on the first `grove init --as mcp-llm`, `init` shells out to
`grove-explore config` to collect the provider, base URL, and model (requires
an interactive terminal), saving them to `.grove/config.json`. Re-runs (when
the config already exists) work without a TTY. When the `grove-explore` binary
isn't found on `PATH`, `init` degrades gracefully — it still registers the
server and prints the follow-up command (`grove-explore config`) instead of
failing the init.

```bash
grove init --as mcp-llm --dry-run   # print planned writes without creating files
```

The skill is distributed through the [agent-skills tool](https://github.com/vercel-labs/skills):

```bash
npx skills add Entelligentsia/grove
```

The skill **prefers grove's MCP tools when the host exposes them and falls back
to the `grove` CLI otherwise** — so MCP and the skill are equal partners over the
same engine. On first CLI use it self-bootstraps: if `grove` isn't on `PATH` it
installs the npm package globally, then runs `grove init --as skill` to fetch the
repo's grammars.

## What `init` writes

| File | Purpose | Target |
|---|---|---|
| `.mcp.json` | registers `grove serve` for MCP-aware harnesses | availability |
| `CLAUDE.md` | a marked steering block routing the agent to grove | adoption |
| `grove.lock` | pinned grammars (version + wasm sha256) | reproducibility |

Re-running `grove init` updates only grove's own pieces and never clobbers
content outside its marked sections.

---

Next: [Languages & grammars](languages.md) · [MCP server](mcp.md)