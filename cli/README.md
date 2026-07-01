# grove (CLI + MCP server)

`grove` gives coding agents **structural, byte-precise, token-cheap access to a
codebase** via tree-sitter — instead of reading whole files. One engine, seven
tools, **two faces**: a human CLI (`grove <verb>`) and an MCP server
(`grove serve`). Grammars load **at runtime from a WASM registry**, so adding a
language needs no recompile.

This crate is the **binary** — a thin `clap` + MCP shell over the
[`grove-cst`](../core/README.md) engine library. Installing it gives you a binary
named **`grove`** (the crates.io package id is `grove-cst-cli` because `grove` is
taken by an unrelated crate).

## Install

```bash
cargo install grove-cst-cli      # installs a binary named `grove`
```

Other channels (curl, Homebrew, npm, the agent skill) and full usage are in the
[project README](../README.md) and [docs](../docs/).

## The tools

`outline` · `symbols` · `source` · `check` · `callers` · `map` · `definition`,
plus `serve` (MCP over stdio) and setup verbs (`init`, `fetch`, `languages`,
`registry`, `lock`). See [docs/tools.md](../docs/tools.md).

## Embedding grove

Driving grove's queries from Rust? Depend on the engine directly — the
[`grove-cst`](../core/README.md) library — instead of shelling out to this
binary.

## License

MIT © the grove authors. Part of the
[grove](https://github.com/Entelligentsia/grove) project.
