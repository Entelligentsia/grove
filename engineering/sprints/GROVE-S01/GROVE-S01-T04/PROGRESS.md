# PROGRESS — GROVE-S01-T04: Curate public API, docs, and standalone library smoke test

## Summary of Changes

Executed the approved PLAN.md to turn `grove-core`'s mechanical module surface into
a deliberate, documented public API and to prove that API from outside the
workspace via a standalone smoke crate.

1. **Curated public surface (`core/src/lib.rs`, AC1).** Expanded the crate-level
   `//!` doc into a usage-oriented overview (a bulleted tour of the `ops` queries
   plus the `init::provision_project` entry point) with a compiled `no_run`
   example, and added a curated block of root `pub use` re-exports so consumers
   can name the common types directly:
   - from `engine`: `Symbol`, `Defect`
   - from `ops`: `CallSite`, `MapEntry`, `FileMap`, `SourceResult`
   - from `init`: `provision_project`

   All six `pub mod` declarations (`engine`/`init`/`ops`/`registry`/`fetch`/
   `ingest`) were kept — the `cli` crate imports `registry`/`fetch`/`ingest`
   directly, so the change is purely additive and non-breaking. Internal helpers
   (`Loaded`, `CapturedQuery`, `Index`, `Sources`, `Spec`, `Catalog`) remain
   private and are NOT re-exported.

2. **Documented public ops (`core/src/ops.rs`, AC2).** Expanded the terse
   one-liner `///` docs on `outline`, `symbols`, `source`, `check`, and
   `definition` into what/params/return form. `callers`, `map`, `definition_at`,
   `parse_pos`, and `project` already carried adequate what/params/return docs and
   were left as-is. The public return structs (`SourceResult`, `CallSite`,
   `MapEntry`, `FileMap`) already documented their fields.

3. **Verified init docs (`core/src/init.rs`, AC2).** `provision_project` already
   carries a docs.rs-quality `///` covering what it does, its `root`/`dry_run`
   params, and the empty-`Vec` short-circuit contract on the return. No edit
   needed — confirmed by reading.

4. **Standalone smoke crate (AC3/AC4).** Created a throwaway crate under
   `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/` that depends on
   `grove-core` purely by relative path and carries an **empty `[workspace]`
   table** to detach from the parent workspace and resolve its own lockfile. Its
   `main.rs` sets `GROVE_REGISTRY` to the repo's checked-in `registry/`, then runs
   `ops::symbols` over a small Rust fixture and `init::provision_project(...,
   dry_run=true)`. A `.gitignore` excludes the compiled `target/` while the
   `Cargo.lock` is committed as durable no-`clap` provenance.

## Grammar resolution (review item 1 — AC3 reproducibility)

`ops::symbols` → `registry::for_path` reads `grammar.wasm` + `tags.scm` from the
resolved registry root. Registry precedence is `GROVE_REGISTRY` env var → user
cache (`~/.cache/grove/grammars/`) → `dev_root()`. For the smoke crate
`dev_root()` resolves to `<smoke>/registry/`, which does **not** exist, so it is
not a usable fallback. To make the smoke test reproducible and independent of
machine-specific cache state, `main.rs` sets `GROVE_REGISTRY` explicitly.

- **Mechanism chosen:** `std::env::set_var("GROVE_REGISTRY", ...)` at the top of
  `main()`, resolving the path from `CARGO_MANIFEST_DIR`'s 5th ancestor (the repo
  root) joined with `registry`.
- **Resolved absolute path (this machine):**
  `/home/boni/src/grove-engineering/grove-GROVE-S01/registry`

## Sample-tree language (review item 2)

The fixture is a single Rust file (`sample/geometry.rs`). Rust is chosen because
the repo's `registry/` ships a verified `rust` grammar
(`registry/rust/grammar.wasm` + `tags.scm` + `manifest.json`, confirmed present),
so `ops::symbols` over `.rs` resolves a grammar guaranteed to exist at the
documented registry path.

## Test Evidence

### Workspace gate — `cargo test --release --locked` (AC5)

```
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s

   Doc-tests grove_core

running 1 test
test core/src/lib.rs - (line 28) - compile ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

The new crate-level `no_run` doctest compiles cleanly alongside the 87 unit tests.

### Lint — `cargo clippy -- -D warnings` (AC5)

```
    Checking grove-core v0.1.11 (.../core)
    Checking grove v0.1.11 (.../cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
```

No warnings (warnings are denied).

### Doc build — `cargo doc -p grove-core --no-deps`

```
 Documenting grove-core v0.1.11 (.../core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.65s
   Generated .../target/doc/grove_core/index.html
```

Builds with no warnings; the `//!`/`///` docs and intra-doc links resolve.

### Library smoke test — `cargo run` in `smoke/` (AC3)

```
GROVE_REGISTRY = /home/boni/src/grove-engineering/grove-GROVE-S01/registry
sample tree   = .../GROVE-S01-T04/smoke/sample

== ops::symbols(sample, defs only) ==
  class      Point        sample/geometry.rs:4
  method     new          sample/geometry.rs:11
  method     distance     sample/geometry.rs:16
  function   perimeter    sample/geometry.rs:24
  (4 definitions)

== init::provision_project(sample, dry_run=true) ==
  detected   rust       1 files

  (dry run — no files written)
  returned 0 wrote-actions (dry-run ⇒ empty)

smoke OK
```

`ops::symbols` returns a non-empty symbol set (4 defs) over the Rust fixture, and
`provision_project(dry_run=true)` narrates the detected language and returns the
contractual empty `Vec` (the evidence is the printed `detected rust 1 files`
narration, not a non-empty vec).

### Clap-free re-confirmation — `cargo tree` + `Cargo.lock` (AC4)

```
$ cargo tree | grep -i clap   → (no match)  →  NO clap node in dependency tree ✓
$ grep 'name = "clap"' Cargo.lock → (no match) →  no clap in Cargo.lock ✓
```

The smoke crate's committed `Cargo.lock` (durable provenance) contains no `clap`
node, re-confirming `grove-core` is CLI-dependency-free.

### Smoke crate detachment / artifact hygiene

- Empty `[workspace]` table in `smoke/Cargo.toml` detaches the crate from the
  parent workspace (`members = ["core", "cli"]`), so it is excluded from the
  published workspace and resolves its own lockfile.
- `smoke/.gitignore` (`/target`) keeps the build tree out of the commit while
  `Cargo.lock` is committed. Verified: `git check-ignore .../smoke/target/`
  matches; the staged set is `.gitignore`, `Cargo.toml`, `Cargo.lock`,
  `src/main.rs`, `sample/geometry.rs`.

## Acceptance Criteria

- [x] **AC1** — `core/src/lib.rs` exposes a curated public surface (`ops`/`init`
      modules plus root re-exports `Symbol`, `Defect`, `CallSite`, `MapEntry`,
      `FileMap`, `SourceResult`, `provision_project`).
- [x] **AC2** — Public `ops` fns and `init::provision_project` carry `///` docs
      covering what/params/return.
- [x] **AC3** — A standalone crate outside the workspace depends on `grove-core`
      by path and successfully calls `ops::symbols` + `provision_project`.
- [x] **AC4** — `cargo tree` and the committed `Cargo.lock` confirm no `clap`
      dependency.
- [x] **AC5** — `cargo test --release --locked` and `cargo clippy -- -D warnings`
      pass at the workspace root.
- [ ] *(Nice-to-have)* `core/README.md` example + `core/examples/` — deferred;
      `core/Cargo.toml` already wires `readme = "../README.md"`. Not required for
      the must-have ACs; left to T06's publish-metadata pass.

## Operational Impact

Material but semver-additive: this changes `grove-core`'s public API additively
(new root re-exports + expanded docs), with no breaking changes — existing
`grove_core::ops::*` / `grove_core::engine::*` paths still resolve. The
version/publish decision is owned by T06. The smoke crate is a verification
fixture and is never published.

## Files Changed

| File | Change |
| --- | --- |
| `core/src/lib.rs` | Expanded crate `//!` doc + `no_run` example; added curated `pub use` re-exports |
| `core/src/ops.rs` | Expanded `///` docs on `outline`/`symbols`/`source`/`check`/`definition` |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/Cargo.toml` | New — path dep on `grove-core`, empty `[workspace]` table |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/Cargo.lock` | New (committed) — no-`clap` provenance |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/src/main.rs` | New — sets `GROVE_REGISTRY`, calls `ops::symbols` + `provision_project` |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/sample/geometry.rs` | New — Rust fixture for `ops::symbols` |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/.gitignore` | New — ignore `/target`, keep `Cargo.lock` |
