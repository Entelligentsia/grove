# ARCHITECT APPROVAL — GROVE-S01-T04

## Curate public API, docs, and standalone library smoke test

**Verdict:** Approved

## Architectural Review

The implementation delivers exactly what the approved plan specified: a curated
public API surface for `grove-core`, docs.rs-quality documentation, and a
standalone smoke crate that proves the library is consumable from outside the
workspace — all without introducing a `clap` dependency.

### Alignment with project architecture

- **Library/CLI separation:** `stack.md` defines `grove-core` as the clap-free
  structural code-intelligence library and `grove` (the binary crate) as the
  CLI host. The T04 changes reinforce this boundary: the curated `pub use`
  re-exports (`Symbol`, `Defect`, `CallSite`, `MapEntry`, `FileMap`,
  `SourceResult`, `provision_project`) give external consumers an ergonomic root
  surface, while the six `pub mod` declarations are preserved so the CLI can
  still reach `registry`/`fetch`/`ingest` directly. This is additive and
  non-breaking.

- **Grammar system:** `stack.md` specifies grammars load as WASM at runtime
  from the registry. The smoke crate sets `GROVE_REGISTRY` to the repo's
  checked-in `registry/` directory (5th ancestor of the smoke manifest dir),
  short-circuiting the cache cascade and making grammar resolution
  reproducible on any machine. The Rust fixture (`sample/geometry.rs`) is
  backed by the verified rust grammar that ships in the repo. This is the
  correct, architecture-consistent approach.

- **Smoke crate detachment:** The empty `[workspace]` table in
  `smoke/Cargo.toml` correctly detaches the crate from the parent workspace,
  giving it its own `Cargo.lock` (committed as durable no-clap provenance)
  without polluting the published workspace member set.

### Acceptance criteria — all verified

| AC | Status | Evidence |
|----|--------|----------|
| AC1 — curated public surface | ✅ | `lib.rs` re-exports 7 types + `provision_project`; all six `pub mod` kept; internal helpers (`Loaded`, `CapturedQuery`, `Index`, `Sources`, `Spec`, `Catalog`) private — not leaked. |
| AC2 — docs.rs-quality docs | ✅ | `ops` fns (`outline`/`symbols`/`source`/`check`/`definition`) expanded to what/params/return form; `provision_project` already adequate; `cargo doc --no-deps` no warnings; `//!` crate doc with `no_run` example compiles as doctest. |
| AC3 — standalone smoke crate | ✅ | Empty `[workspace]` table detaches; `ops::symbols` returns 4 defs from Rust fixture; `provision_project(dry_run=true)` returns Ok; `GROVE_REGISTRY` set to repo `registry/` — reproduced independently. |
| AC4 — no clap | ✅ | `grep -c clap Cargo.lock` → 0; `cargo tree` shows no clap node. Smoke `Cargo.lock` committed as durable provenance. |
| AC5 — workspace gate | ✅ | `cargo test --release --locked` = 87 passed + 1 doctest; `cargo clippy -D warnings` clean; `cargo doc -p grove-core --no-deps` no warnings. |

### Cross-cutting concerns

- **No impact on other modules:** All changes are additive (new re-exports, doc
  expansions, new smoke crate files). No existing behavior is altered.
- **No migration required:** The public API additions are semver-additive.
  Version/publish decision is correctly deferred to T06.
- **Advisory from code review:** A stray root `.gitignore` change
  (`.pi/pi-claude-compat/`) and unrelated `cli/src/*`/`src/*` deletions exist in
  the working tree from other work. These are NOT T04 changes. The commit-phase
  staging set must scope to T04's `files_changed` provenance only — these
  stray paths must not be swept into the T04 commit.

## Deployment Notes

- **Materiality:** Material but semver-additive. No end-user action required.
- **Distribution:** No binary, packaging, or distribution changes. The smoke
  crate is `publish = false` and never shipped.
- **Version bump:** Not required in this task. The version/publish decision is
  owned by T06.

## Follow-Up Items

1. **T06 (version/publish):** Decide whether the additive public-API surface
   warrants a minor version bump when `grove-core` is next published.
2. **Nice-to-have (deferred):** `core/README.md` with the `analyze_project`
   snippet and `core/examples/analyze_project.rs` — explicitly deferred to T06
   per the approved plan. Not a must-have for this task.
3. **Commit hygiene:** Ensure the commit-phase staging set excludes the stray
   root `.gitignore` and unrelated `cli/src/*`/`src/*` working-tree changes
   flagged in the code review advisory.