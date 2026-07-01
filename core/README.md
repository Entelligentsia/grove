# grove-core

The structural code-intelligence library behind the [grove](https://github.com/Entelligentsia/grove)
CLI and MCP server. `grove-core` hosts the tree-sitter **AST engine**, the
**grammar registry**, grammar **fetch**, and source **ingest** — the same engine
the `grove` binary drives. It gives you **structural, byte-precise, token-cheap
access to a codebase**: query definitions, sources, callers, and dependency maps
without reading whole files.

Grammars load **at runtime from a hosted WASM registry**, so no grammar is
compiled in and adding a language needs no recompile. The crate is **`clap`-free**
— command-line concerns live in the `grove` binary, not here.

## Install

Published on crates.io as **`grove-cst`** — CST for the *concrete syntax trees*
tree-sitter builds (the plain `grove-core` name belongs to an unrelated crate).
The library name is still `grove_core`, so alias it and your imports stay
unchanged:

```toml
[dependencies]
grove_core = { package = "grove-cst", version = "0.1" }
```

Before the first crates.io release, depend on it by git instead:

```toml
[dependencies]
grove_core = { git = "https://github.com/Entelligentsia/grove", package = "grove-cst" }
```

## Usage

The consumer-facing surface is the [`ops`] module — a small set of structural
queries that work for **any** registered language. Before querying, provision the
grammars for the target project once with [`init::provision_project`]: it detects
the project's languages, **fetches any missing grammar into the OS cache**, and
pins `grove.lock`. After that, every `ops::*` call resolves grammars from the
cache.

```rust
use std::path::Path;
use grove_core::{init, ops};

fn main() -> anyhow::Result<()> {
    let project = Path::new(".");

    // 1. Provision grammars for the languages in this project. Fetches any
    //    missing grammar into the OS cache and pins grove.lock. Run once;
    //    pass `true` for a dry run (detect only, no network, no writes).
    for action in init::provision_project(project, false)? {
        println!("provisioned: {action}");
    }

    // 2. Query — grammars now resolve from the cache. Every definition under
    //    `src/`, gitignore-aware.
    for s in ops::symbols(&project.join("src"), None, None, false, false)? {
        println!("{} {} — {}:{}", s.kind, s.name, s.file, s.line);
    }

    // 3. One symbol's full source, by name — no whole-file read.
    let hit = ops::source("src/lib.rs", Some("main"))?;
    println!("{}", hit.source);

    Ok(())
}
```

> **Offline / pinned registry:** to skip the network and resolve grammars from a
> specific registry root instead, set `GROVE_REGISTRY=<dir>` (highest resolution
> precedence) and call the `ops::*` functions directly — provisioning is only
> needed to populate the cache.

### Manual grammar management

`provision_project` is the batteries-included path (detect → fetch → lock). If
you want direct control, the [`fetch`] and [`registry`] modules expose each step:

```rust
use std::path::Path;
use grove_core::{fetch, registry};

fn main() -> anyhow::Result<()> {
    // Discover what the hosted registry offers.
    for g in fetch::catalog_grammars()? {
        println!("{:<12} {:?}", g.name, g.extensions);
    }

    // Fetch specific grammars into the OS cache (pass `true` to re-download).
    let langs = vec!["rust".to_string(), "python".to_string()];
    fetch::run(&langs, false)?;

    // What's resolvable now (cache + any GROVE_REGISTRY / project registry)?
    println!("available: {}", registry::available().join(", "));
    println!("registry root: {}", registry::root().display());

    // Register (load + compile) one grammar by name — cached per process.
    let rust = registry::resolve("rust")?;
    println!("loaded {} v{}", rust.name, rust.version);

    // …or resolve the grammar for a given file by its extension.
    let g = registry::for_path(Path::new("src/lib.rs"))?;
    println!("src/lib.rs → {}", g.name);

    // Pin the resolved set into grove.lock (version + wasm sha256).
    let n = registry::write_lock_for(&langs, Path::new("grove.lock"))?;
    println!("locked {n} grammars");

    Ok(())
}
```

`registry::resolve` and `for_path` return a `Grammar` (the loaded wasm + compiled
`tags.scm` + profile), cached per process. `registry::search_path()` shows the
full resolution precedence, and `registry::write_lock` / `locked_langs` read and
write `grove.lock` for reproducible pins.

### The surface

| Function | Returns |
|---|---|
| `ops::outline` | the definitions in one file (its symbol skeleton) |
| `ops::symbols` | find symbols across a directory, gitignore-aware |
| `ops::source` | the full source text of one symbol, by id or name |
| `ops::check` | the syntactic defects (`ERROR` / `MISSING`) in one file |
| `ops::callers` | every reference to a name, with its enclosing function |
| `ops::map` | a directory's definitions and their outgoing references |
| `ops::definition` / `ops::definition_at` | go-to-def by name or from a use site |

Return types — `Symbol`, `Defect`, `CallSite`, `FileMap`, `MapEntry`,
`SourceResult` — are re-exported at the crate root (e.g. `grove_core::Symbol`).
[`init::provision_project`] is the grammar-provisioning entry point behind
`grove init`. The lower-level `engine`, `registry`, `fetch`, and `ingest` modules
are public for hosts that need deeper access.

Every result carries a stable `symbol-id` (`<lang>:<relpath>#<name>@<line>`,
1-based) you can pass between calls.

## Grammars & the registry

`grove-core` resolves grammars from the first existing location (precedence):
`GROVE_REGISTRY` env → `<project>/.grove/grammars/` → the OS cache
(`~/.cache/grove/grammars` on Linux) → a dev `registry/` tree. Set
`GROVE_REGISTRY` to point at a specific registry root for reproducible resolution.
See the [grove docs](https://github.com/Entelligentsia/grove/blob/main/docs/languages.md)
for the WASM registry, profiles, and the 27 supported languages.

## Not an LSP

grove is a syntactic, tree-sitter-powered layer — it parses and locates, it does
not do type inference, completion, rename, or type-resolved go-to-def. It is the
cheap syntactic layer *beneath* where an LSP's semantics begin.

## License

MIT © the grove authors. Part of the [grove](https://github.com/Entelligentsia/grove)
project.

[`ops`]: https://github.com/Entelligentsia/grove/blob/main/core/src/ops.rs
[`fetch`]: https://github.com/Entelligentsia/grove/blob/main/core/src/fetch.rs
[`registry`]: https://github.com/Entelligentsia/grove/blob/main/core/src/registry.rs
[`init::provision_project`]: https://github.com/Entelligentsia/grove/blob/main/core/src/init.rs
