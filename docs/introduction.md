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

grove is a single Rust binary and a library over one engine. You can reach it
four ways:

| Surface | What it is | Start here |
|---|---|---|
| **CLI** | `grove <verb>` — the seven tools at your shell, human tables or `--json` | [CLI & the seven tools](tools.md) |
| **MCP: standard** | `grove serve` — the same seven tools to a coding agent over stdio | [MCP: standard server](mcp.md) |
| **MCP: explore** | `grove serve --explore` — a single delegated `explore` locator backed by a local LLM | [MCP: explore mode](setup.md) |
| **Library** | `grove-cst` on crates.io — `use grove_core::ops` in your own Rust | [Use grove as a library](library.md) |

All four call the same `ops` engine, so a human at the shell, an agent over MCP,
and your own program see identical results.

## Get going

- **[Install](install.md)** — curl / Homebrew / npm / cargo, or build from source.
- **[Setup](setup.md)** — `grove init` wires a project for your agent
  (`--as mcp | skill | both | mcp-llm`).
- **[CLI & tools](tools.md)** — the seven-tool surface and its conventions.
- **[Languages & grammars](languages.md)** — the WASM registry and how grammars load.

New to why this matters? The project [VISION](https://github.com/Entelligentsia/grove/blob/main/VISION.md)
and [README](https://github.com/Entelligentsia/grove/blob/main/README.md) tell the
longer story; the [FAQ](faq.md) answers the common "is this an LSP?" questions.
