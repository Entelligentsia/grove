# PROGRESS — GROVE-S01-T02: Extract grove-core library; rewire CLI to consume it

## Summary of Changes

Executed the approved PLAN.md verbatim: extracted the engine, ops, registry,
fetch, and ingest modules out of the `grove` binary crate into a new
`grove-core` library crate, and rewired the three CLI consumers
(`main.rs`, `mcp.rs`, `init.rs`) to source those modules via `grove_core::`
instead of bare `crate::` module declarations.

This is a pure mechanical extraction + path rewire — no engine logic was
rewritten. Both prior review revisions were honoured:

- **REVISION 1** — `main.rs`'s import group is
  `use grove_core::{ops, registry, fetch, ingest};` with `engine` deliberately
  excluded (zero `engine::` path refs in `main.rs`; `engine` is consumed
  internally by `ops` inside `grove-core`). Importing it would be a dead
  `unused_imports` warning that `clippy -D warnings` (AC#7) promotes to a hard
  error. Confirmed clippy clean below.
- **REVISION 2** — `core/Cargo.toml`'s `serde` dependency carries
  `features = ["derive"]`; the moved modules (`registry.rs`, `ops.rs`) use
  `#[derive(Serialize)]` / `#[derive(Deserialize)]`. Build is green, proving the
  feature resolved.

### Work performed

1. **Scaffolded `core/`**
   - `core/Cargo.toml` — package `grove-core`, edition 2021, with the engine's
     full dependency set: `tree-sitter` (wasm), `streaming-iterator`,
     `serde` (derive), `serde_json`, `ignore`, `anyhow`, `sha2`, `dirs`, `ureq`.
     **No `clap`.**
   - `core/src/lib.rs` — `pub mod engine; pub mod ops; pub mod registry;
     pub mod fetch; pub mod ingest;` plus crate-level docs.
2. **Registered the crate** — root `Cargo.toml` `members = ["cli"]` →
   `["core", "cli"]`.
3. **Moved five modules with history** — `git mv cli/src/{engine,ops,registry,fetch,ingest}.rs
   core/src/`. Their internal `crate::` cross-references
   (`crate::engine`, `crate::registry`, `crate::{fetch, registry}` in `ingest.rs`)
   remain intra-crate once co-located in `grove-core`, so no path edits were
   needed inside the moved set. (Verified before moving: every `crate::` ref in
   the moved set targets another moved module; the only other match was a string
   literal in an `ops.rs` test assertion.)
4. **Rewired the three CLI consumers**
   - `cli/src/main.rs` — dropped the five `mod engine/fetch/ingest/ops/registry;`
     decls, kept `mod init; mod mcp;`, and added
     `use grove_core::{ops, registry, fetch, ingest};`.
   - `cli/src/mcp.rs` — `use crate::{ops, registry};` →
     `use grove_core::{ops, registry};`.
   - `cli/src/init.rs` — `use crate::{fetch, registry};` →
     `use grove_core::{fetch, registry};`.
5. **Partitioned dependencies** — `cli/Cargo.toml` now declares
   `grove-core = { path = "../core" }` and retains only the crates the CLI uses
   directly (`clap`, `serde` (derive), `serde_json`, `ignore`, `anyhow`); the
   core-only deps (`tree-sitter`, `streaming-iterator`, `sha2`, `dirs`, `ureq`)
   were dropped from the CLI manifest. `clap` lives **only** in the CLI crate.
6. **Regenerated `Cargo.lock`** — the new `grove-core` package entry required a
   lockfile update (offline; all transitive deps already present), after which
   the full workspace builds under `--locked`.

## Test Evidence

### Incremental core-first compile (`cargo build --release -p grove-core`)

```
   Compiling grove-core v0.1.11 (/home/.../grove-GROVE-S01/core)
    Finished `release` profile [optimized] target(s) in 0.80s
```

### Workspace build (`cargo build --release --locked --workspace`)

```
   Compiling grove v0.1.11 (/home/.../grove-GROVE-S01/cli)
    Finished `release` profile [optimized] target(s) in 10.97s
```

### Workspace tests (`cargo test --release --locked --workspace`)

```
     Running unittests src/main.rs (.../deps/grove-2ac8f40db4052d81)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cli.rs (.../deps/cli-4af1961204ea343c)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (.../deps/grove_core-0f1eb58767f765db)
test result: ok. 85 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests grove_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Total: **134 tests passed, 0 failed.** All pre-existing tests survive the split
(85 unit tests now live with the moved modules in `grove-core`; 32 binary unit
tests + 17 integration tests remain in `cli`).

### Lint (`cargo clippy --workspace --locked -- -D warnings`)

```
    Checking grove-core v0.1.11 (.../core)
    Checking grove v0.1.11 (.../cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.80s
```

Exit code 0 — clean, no warnings promoted to errors. Confirms REVISION 1's
`engine`-exclusion (no `unused_imports`).

### Clap-free bar (AC#4 — `cargo tree -p grove-core | grep -i clap`)

```
NO CLAP (good)
```

`grove-core`'s dependency tree contains no `clap` anywhere — command-line
concerns are isolated to the binary crate.

### Binary identity (`grove --version`, `grove --help`)

```
grove 0.1.11
```

Binary is still named `grove` (`target/release/grove`), version unchanged, and
`grove --help` lists all 17 subcommands (outline, symbols, source, check,
callers, map, definition, init, fetch, ingest, index, registry, languages, lock,
serve, …) — no surface change.

### Dev-stub / registry count unchanged (`grove languages`)

```
21 language(s) in registry
```

Registry data was untouched by this refactor; the language count is unchanged.

## Files Changed Manifest

New (untracked):
- `core/Cargo.toml` — new `grove-core` library manifest (engine deps, no clap).
- `core/src/lib.rs` — public module surface for the library.

Modified:
- `Cargo.toml` (root) — workspace `members = ["core", "cli"]`.
- `cli/Cargo.toml` — added `grove-core` path dep; trimmed core-only deps.
- `cli/src/main.rs` — dropped 5 `mod` decls; added `use grove_core::{…}` group.
- `cli/src/mcp.rs` — `crate::` → `grove_core::` import.
- `cli/src/init.rs` — `crate::` → `grove_core::` import.
- `Cargo.lock` — regenerated for the new `grove-core` package entry.

Moved (git rename, history preserved) — `cli/src/` → `core/src/`:
- `engine.rs`, `ops.rs`, `registry.rs`, `fetch.rs`, `ingest.rs`.

## Acceptance Criteria

- [x] `core/` package `grove-core` exists, edition 2021, in root workspace members.
- [x] `engine.rs`, `ops.rs`, `registry.rs`, `fetch.rs`, `ingest.rs` physically moved to `core/src/`.
- [x] `cli/Cargo.toml` declares `grove-core = { path = "../core" }`; `main.rs`/`mcp.rs`/`init.rs` consume via `grove_core::`.
- [x] `cargo tree -p grove-core` shows no `clap`.
- [x] `core/Cargo.toml` carries only the deps core uses; `serde` has `features = ["derive"]`.
- [x] `cli/src/main.rs`'s `grove_core` import group omits `engine` (unused there).
- [x] `cargo build --release --locked --workspace` and `cargo test … --workspace` green.
- [x] `cargo clippy --workspace -- -D warnings` clean.
- [x] Binary still named `grove`; dev-stub / language counts unchanged.

## Knowledge Writeback

No new discoveries that contradict or extend the KB — the live codebase matched
the plan's pre-verified claims exactly (intra-core `crate::` refs, serde derive
usage, dep partition, `engine`-free `main.rs`). No architecture/stack doc updates
required for this mechanical extraction. The `init.rs` provisioning split and the
remaining `crate::`→`grove_core::` follow-ons are explicitly deferred to T03 per
the plan.
