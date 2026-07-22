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
grove init --as mcp-llm  # locator only: grove-explore under the grove key + CLAUDE.md + AGENTS.md
```

## Explore-mode — `grove init --as mcp-llm`

> **Opt-in, and exclusive with the structural surface.** `grove-explore` is a
> separate binary with its own MCP server; `--as mcp-llm` registers it **instead
> of** the structural `grove serve`, not alongside it (see
> [ADR 0005](adr/0005-exclusive-mcp-surfaces.md)). The `.grove/config.json`
> config format and the `explore` tool contract are covered by semantic
> versioning as of 0.3.0. The standard `--as mcp|skill|both` targets and the
> always-structural 7-tool `grove serve` are unaffected either way.

`--as mcp-llm` registers the **locator alone** — the `grove-explore` binary,
under the `.mcp.json` key `grove`, exposing a single tool `mcp__grove__explore`
backed by a local LLM (configured via `.grove/config.json`'s `explore`
section). It replaces the structural surface rather than composing with it: a
project registers `grove` (structural) **or** the locator, never both (see
[ADR 0005](adr/0005-exclusive-mcp-surfaces.md)). The outer agent asks
`mcp__grove__explore` narrow *where-is* questions and reads a window at each
cited `file:line` itself. If the provider is unreachable, `grove-explore serve`
starts anyway and the `explore` tool returns an actionable error in-band —
there is no silent fallback to the structural tools.

The locator uses the `grove` key deliberately, so the tool is
`mcp__grove__explore` — the historical name and the one the outer agent should
see; the `grove-explore` binary is a packaging detail. It's safe because the
surfaces are exclusive, so the key is never contended.

What it writes:

- **`.mcp.json`** — one MCP server under the `grove` key, running
  `grove-explore serve` (any stale two-server-era `grove-explore` key is
  cleaned up on init).
- **`CLAUDE.md`** — a locator-framed steering block: ask `mcp__grove__explore`
  broad *where-is* questions, then read the cited lines.
- **`AGENTS.md`** — the same steering, harness-neutral, for non-Claude
  harnesses (Codex, Cline, etc.).

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