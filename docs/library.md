# Use grove as a library

The engine behind the CLI and the MCP server is a published Rust crate,
**[`grove-cst`](https://crates.io/crates/grove-cst)** (the library name is
`grove_core`). Depend on it to run the same structural queries in-process — no
subprocess, and the grammar cache stays warm across calls.

> The crates.io id is `grove-cst` (`grove` and `grove-core` were taken by
> unrelated crates); the library you `use` is `grove_core`, and the CLI binary
> is `grove` (published as `grove-cst-cli`).

## Add it

```bash
cargo add grove-cst
```

## The `ops` surface

Every structural query lives in [`grove_core::ops`] and works for any registered
language (grammars load from the registry as WASM at runtime):

```rust
use std::path::Path;
use grove_core::ops;

fn main() -> anyhow::Result<()> {
    // Every definition under `src/`, gitignore-aware.
    for s in ops::symbols(Path::new("src"), None, None, false, false)? {
        println!("{} {} — {}:{}", s.kind, s.name, s.file, s.line);
    }

    // One symbol's full source, by id or by (file, name).
    let body = ops::source("rust:src/main.rs#main@1", None)?;
    println!("{}", body.source);
    Ok(())
}
```

The core return types (`Symbol`, `SourceResult`, `CallSite`, `FileMap`, `Defect`)
are re-exported at the crate root. To print results the way the CLI does, use
[`grove_core::render`] — the same formatters both faces share (see
[ADR 0003](adr/0003-shared-verb-rendering-in-core.md)).

## Provisioning grammars

A consumer that ships its own environment can provision the grammars a project
needs (fetch + write `grove.lock`) via [`grove_core::init::provision_project`],
or resolve them explicitly through the [`grove_core::registry`] module. The
registry-resolution precedence is described in [Languages & grammars](languages.md).

## API reference

The full, always-current API reference is published on **docs.rs**:

- **<https://docs.rs/grove-cst>** — every public item, generated from the source.

[`grove_core::ops`]: https://docs.rs/grove-cst/latest/grove_core/ops/index.html
[`grove_core::render`]: https://docs.rs/grove-cst/latest/grove_core/render/index.html
[`grove_core::init::provision_project`]: https://docs.rs/grove-cst/latest/grove_core/init/fn.provision_project.html
[`grove_core::registry`]: https://docs.rs/grove-cst/latest/grove_core/registry/index.html
