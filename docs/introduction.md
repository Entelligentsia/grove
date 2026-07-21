# grove

**Structural, byte-precise, token-cheap access to a codebase — for coding agents
and the humans working alongside them.**

Instead of reading whole files or grepping blind, grove uses
[tree-sitter](https://tree-sitter.github.io/) to answer *structural* questions:
what's defined in a file, where a symbol lives, who calls it, how a directory
connects. Every answer is one symbol's worth of structure with a stable id you
can pass to the next call. Grammars load at runtime from a hosted WASM registry,
so a new language is a dropped-in directory — no recompile, no toolchain on the
consumer.

## One engine, four surfaces

grove is a library (`grove-cst`) over one structural engine, shipped as **two
binaries** — `grove` (CLI + the always-structural MCP server) and
`grove-explore` (its own MCP server, the LLM-delegating locator). You can
reach the engine four ways:

| Surface | What it is | Start here |
|---|---|---|
| **CLI** | `grove <verb>` — the seven tools at your shell, human tables or `--json` | [CLI & the seven tools](tools.md) |
| **MCP: structural** | `grove serve` — the same seven tools to a coding agent over stdio | [MCP: standard server](mcp.md) |
| **MCP: explore** | `grove-explore serve` — a single delegated `explore` locator backed by a local LLM, its own server identity, composable alongside `grove serve` | [MCP: explore mode](setup.md) |
| **Library** | `grove-cst` on crates.io — `use grove_core::ops` in your own Rust | [Use grove as a library](library.md) |

The CLI, MCP structural, and Library surfaces all call the same `ops` engine, so
a human at the shell, an agent over MCP, and your own program see identical
results. MCP structural and MCP explore are two separate servers a project can
register independently or together — not a mode switch on one server.

## Get going

- **[Install](install.md)** — curl / Homebrew / npm / cargo, or build from source.
- **[Setup](setup.md)** — `grove init` wires a project for your agent
  (`--as mcp | skill | both | mcp-llm`).
- **[CLI & tools](tools.md)** — the seven-tool surface and its conventions.
- **[Languages & grammars](languages.md)** — the WASM registry and how grammars load.

New to why this matters? The project [VISION](https://github.com/Entelligentsia/grove/blob/main/VISION.md)
and [README](https://github.com/Entelligentsia/grove/blob/main/README.md) tell the
longer story; the [FAQ](faq.md) answers the common "is this an LSP?" questions.
