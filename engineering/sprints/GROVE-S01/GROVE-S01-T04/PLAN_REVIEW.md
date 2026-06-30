# PLAN REVIEW — GROVE-S01-T04: Curate public API, docs, and standalone library smoke test

*grove Supervisor* · (standalone review)

---

**Verdict:** Approved

This is a re-review after a prior `Revision Required` verdict flagged two critical
items: (1) `dev_root()` resolves to `core/registry/` which does not exist, and
(2) grammar resolution for `ops::symbols` depended on machine-specific user-cache
state. Both items are now fully addressed in the revised plan. Every load-bearing
claim has been independently verified against the actual source.

---

## Prior Revision Items — Resolution

### Item 1: dev_root() resolves to a non-existent path

**Resolved.** The plan now explicitly documents that `dev_root()` returns
`concat!(env!("CARGO_MANIFEST_DIR"), "/registry")` — verified in
`core/src/registry.rs:120`. For `grove-core` this is `core/registry/` and for the
smoke crate it is `<smoke>/registry/` — **neither exists** (confirmed: `core/registry/`
is absent on disk). The plan no longer relies on `dev_root()` as a fallback. Instead
it sets `GROVE_REGISTRY` to the repo-root `registry/` directory, which
`registry_root()` (line 160) checks first and returns immediately, bypassing the
entire `search_path()` cascade. The repo-root `registry/` exists and contains
`rust/grammar.wasm` + `rust/tags.scm` + `rust/manifest.json` (verified on disk).

### Item 2: Grammar resolution via user cache (machine-specific dependency)

**Resolved.** The plan sets `GROVE_REGISTRY` (via `std::env::set_var` at the top of
`main.rs`, or via the run-command prefix) to the repo-root `registry/` before any
`ops::symbols` call. Traced the full call chain:

- `ops::symbols(dir, ...)` → `for_each_source` (ops.rs:48) → `registry::for_path` (line 261) → `registry::resolve` (line 210) → `index()` (line 179) → `registry_root()` (line 160).
- `registry_root()` returns the `GROVE_REGISTRY` value directly.
- `index()` reads manifests from that root via `std::fs::read_dir`; `rust/manifest.json` registers `.rs` → `rust`.
- `resolve("rust")` reads `registry/rust/grammar.wasm` and `tags.scm` from the same root.

This makes the smoke test fully reproducible and independent of user-cache state.
The `index()` `OnceLock` is initialized on first access — since `GROVE_REGISTRY` is
set before any registry call, the first initialization picks it up correctly.

### Sample-tree language (Rust)

**Verified.** `registry/rust/` ships `grammar.wasm` (1.1 MB), `tags.scm`, `locals.scm`,
and `manifest.json`. Rust is the correct choice for a self-evident, repo-bundled
grammar fixture.

---

## Independent Verification Summary

| Claim | Source Location | Verified |
|---|---|---|
| `Symbol`, `Defect` are `pub` in `engine` | engine.rs:22, :48 | ✓ |
| `CallSite`, `MapEntry`, `FileMap`, `SourceResult` are `pub` in `ops` | ops.rs:229, :401, :416, :165 | ✓ |
| `provision_project` is `pub` in `init` | init.rs:29 | ✓ |
| Internal helpers (`Loaded`, `CapturedQuery`, `Index`, `Sources`, `Spec`, `Catalog`) are private (no `pub`) | engine.rs:68, :84; registry.rs:173; ingest.rs:18, :24; fetch.rs:24 | ✓ |
| `core/Cargo.toml` has no `clap` dependency | Cargo.toml deps section | ✓ |
| Registry is at repo root, not `core/registry/` | `ls registry/` — exists; `ls core/registry/` — absent | ✓ |
| `registry/rust/` has grammar.wasm + tags.scm + manifest.json | confirmed on disk | ✓ |
| `dev_root()` = `CARGO_MANIFEST_DIR/registry` (compile-time, non-existent for both crates) | registry.rs:120 | ✓ |
| `GROVE_REGISTRY` short-circuits `registry_root()` | registry.rs:160-162 | ✓ |
| `provision_project(dry_run=true)` returns `Ok(Vec::new())`, needs no grammar/WASM | init.rs:29-75 — dry_run branch returns before any fetch/resolve | ✓ |
| CLI imports `registry`, `fetch`, `ingest` directly (keeping `pub mod` is necessary) | cli/src/main.rs: `use grove_core::{ops, registry, fetch, ingest}` | ✓ |
| Empty `[workspace]` table detaches smoke crate from parent workspace | standard Cargo mechanism | ✓ |

---

## Advisory Notes (non-blocking)

1. **Registry precedence prose is slightly incomplete.** The plan states the
   precedence as "GROVE_REGISTRY → user cache → dev_root()". The actual
   `search_path()` (registry.rs:132) inserts a `.grove/grammars` project-level
   candidate between `GROVE_REGISTRY` and user cache. This is immaterial to the
   smoke test (GROVE_REGISTRY short-circuits the search), but the plan's
   explanatory prose should mention the `.grove/grammars` step for accuracy when
   the engineer writes PROGRESS documentation.

2. **GROVE_REGISTRY path construction.** The smoke crate's `CARGO_MANIFEST_DIR`
   is `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/`. The repo-root
   `registry/` is six levels up. Whether the engineer uses
   `concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../../../registry")` (absolute
   path with `..` components, which `std::fs` resolves correctly) or an env-prefix
   on the run command, the exact resolved absolute path must be recorded in PROGRESS
   as the plan commits. The `index()` `OnceLock` means `GROVE_REGISTRY` must be set
   before the first registry call — the plan correctly specifies this.

3. **`extension_map()` network call.** `provision_project` calls
   `fetch::catalog_grammars()` (init.rs:97) which hits the hosted catalog online.
   On failure it falls back to `registry::manifests()`, which reads from
   `registry_root()` (the `GROVE_REGISTRY` path) — so `.rs` files will be detected
   as `rust` even offline. The dry-run path prints the detection narration and
   returns `Ok(Vec::new())` before any fetch attempt. No action needed; noting for
   PROGRESS completeness (the `note: catalog unavailable` stderr line may appear
   if offline — this is expected, not an error).

---

## Conclusion

The plan is feasible, complete, and correctly addresses both prior revision items.
The additive strategy (keep `pub mod`, add `pub use`) is non-breaking and preserves
CLI compatibility. The smoke test design is now reproducible and machine-independent
via `GROVE_REGISTRY`. All acceptance criteria are achievable as specified. No
blocking issues remain.