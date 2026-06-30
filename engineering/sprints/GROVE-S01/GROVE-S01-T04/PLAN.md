# PLAN — GROVE-S01-T04: Curate public API, docs, and standalone library smoke test

🌱 *grove Engineer*

**Task:** GROVE-S01-T04
**Sprint:** GROVE-S01
**Estimate:** M

---

## Objective

Promote the mechanically-exposed `grove-core` modules into a deliberate, documented
public API and prove it works from outside the workspace. `core/src/lib.rs` will
carry a crate-level `//!` doc and curated root re-exports of the consumer-facing
items (`ops`, `init::provision_project`, and the return types `Symbol`, `Defect`,
`CallSite`, `MapEntry`, `FileMap`, `SourceResult`). The public `ops` and `init`
functions gain `///` docs sufficient to render on docs.rs. A standalone crate
*outside* the Cargo workspace, depending on `grove-core` by path, compiles and runs
`ops::symbols(...)` and `init::provision_project(...)` against a sample tree —
the concrete "Library smoke test" evidence for issue #50, with `cargo tree`
re-confirming the public surface pulls in no `clap`.

## Approach

1. **Curate the surface in `core/src/lib.rs`.** Keep the existing `pub mod`
   declarations, expand the crate-level `//!` doc into a short usage-oriented
   overview, and add a curated block of root re-exports so consumers can write
   `grove_core::Symbol` / `grove_core::provision_project` instead of reaching
   through module paths. Re-export the return types a caller needs:
   - from `engine`: `Symbol`, `Defect`
   - from `ops`: `CallSite`, `MapEntry`, `FileMap`, `SourceResult`
   - from `init`: `provision_project`
   Use `pub use` (optionally with a brief `///` on each re-export group). Do not
   leak internal-only helpers (`Loaded`, `CapturedQuery`, `Index`, `Sources`,
   `Spec`, `Catalog`, etc.) — those stay private to their modules.
2. **Document public items.** Add/expand `///` doc comments on the public `ops`
   functions (`symbols`, `outline`, `source`, `check`, `callers`, `map`,
   `definition`, `definition_at`, `project`, `parse_pos`) and on the public structs
   they return, plus confirm `init::provision_project` already has a usable doc
   (it does — verify and lightly polish). Each public fn doc states what it does,
   its parameters, and its return. Reserve deeper prose for the load-bearing entry
   points consumers will call first (`symbols`, `provision_project`).
3. **Build the standalone smoke crate.** Create a throwaway crate under
   `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/` with its own `Cargo.toml`
   that (a) depends on `grove-core` via a relative `path`, and (b) carries an empty
   `[workspace]` table so it detaches from the parent workspace and resolves its
   own lockfile. `main.rs` calls `grove_core::ops::symbols(...)` over a small
   **Rust** sample source tree and calls
   `grove_core::init::provision_project(sample_root, /*dry_run*/ true)`, printing
   the results. Record exact commands and observed output in PROGRESS.
   - **Sample-tree language (review item 2):** the sample tree contains a small
     `.rs` fixture. Rust is chosen because the repo's `registry/` ships a `rust`
     grammar (`registry/rust/grammar.wasm` + `tags.scm`, verified present), so
     `ops::symbols` over `.rs` files resolves a grammar that is guaranteed to exist
     at the documented registry path. (Python `.py` and JavaScript `.js` are also
     available, but Rust keeps the fixture self-evident.)
   - **Grammar resolution (review item 1, AC3 reproducibility):** `ops::symbols`
     calls `registry::for_path()` → `registry::resolve()`, which reads
     `grammar.wasm` + `tags.scm` from the resolved registry root. The registry
     precedence is: explicit `GROVE_REGISTRY` env var → user cache
     (`~/.cache/grove/grammars/`) → `dev_root()`. Note that `dev_root()` resolves
     to `concat!(env!("CARGO_MANIFEST_DIR"), "/registry")`, which for the smoke
     crate is `<smoke>/registry/` (and for `grove-core` is `core/registry/`) —
     **neither exists**, so `dev_root()` is NOT a usable fallback here. To make the
     smoke test reproducible and independent of machine-specific user-cache state,
     the smoke crate sets `GROVE_REGISTRY` to the repo's root `registry/` directory
     before calling `ops::symbols` — either via `std::env::set_var("GROVE_REGISTRY",
     ...)` at the top of `main.rs` (resolving the path relative to the smoke
     crate's `CARGO_MANIFEST_DIR`) or by prefixing the documented run command with
     `GROVE_REGISTRY=<repo>/registry`. The chosen mechanism and the resolved
     absolute path are recorded in PROGRESS. (`provision_project(dry_run=true)`
     needs no grammar — it only counts files by extension and returns
     `Ok(Vec::new())` after printing its narration.)
4. **Re-confirm clap-free.** Run `cargo tree` in the smoke crate and assert no
   `clap` node appears; **commit** the smoke crate's `Cargo.lock` showing the same
   so the no-`clap` evidence is durable provenance rather than ephemeral state.
5. **Keep the workspace green.** Run the configured `cargo test --release --locked`
   and `cargo clippy -- -D warnings` at the workspace root after the lib.rs/doc
   edits; the smoke crate is built/run separately (it is not a workspace member).

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `core/src/lib.rs` | Expand `//!` crate doc; add curated `pub use` re-exports of `Symbol`, `Defect`, `CallSite`, `MapEntry`, `FileMap`, `SourceResult`, `provision_project` | Turn mechanical `pub mod`s into a deliberate, ergonomic public surface (AC1) |
| `core/src/ops.rs` | Expand the existing terse one-liner `///` docs on public fns (e.g. `symbols`) into what/params/return form, and on return structs | docs.rs-quality documentation for the consumer-facing operations (AC2) |
| `core/src/init.rs` | Verify/polish `///` on `provision_project` | Ensure the init entry point documents params + return (AC2) |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/Cargo.toml` | New — standalone crate, `grove-core` path dep, empty `[workspace]` table | Out-of-workspace consumer proving the library API (AC3, AC4) |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/src/main.rs` | New — sets `GROVE_REGISTRY` to the repo `registry/`, then calls `ops::symbols` (Rust fixture) + `init::provision_project` on the sample tree | Executable, reproducible smoke evidence (AC3) |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/Cargo.lock` | New (committed) — generated lockfile showing no `clap` node | Durable AC4 provenance |
| `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/sample/*.rs` | New — small Rust fixture tree `ops::symbols` runs against | Grammar-backed input for the smoke test (AC3) |
| `core/README.md` *(nice-to-have)* | New — crate-level README with the `analyze_project` snippet, wired as `readme` in `core/Cargo.toml` | Feeds T06 publish metadata; only if must-haves complete |
| `core/examples/analyze_project.rs` *(nice-to-have)* | New — example mirroring the snippet | Rounds out docs.rs presentation; only if must-haves complete |

## Data Model Changes

None. No store schema, config, or persistent data structures change. The work is
confined to the Rust public API surface (re-exports + doc comments) and a
verification-only crate. The public return types (`Symbol`, `Defect`, `CallSite`,
`MapEntry`, `FileMap`, `SourceResult`) are re-exported, not redefined.

## Testing Strategy

- **Workspace gate:** `cargo test --release --locked` and `cargo clippy -- -D warnings`
  at the workspace root stay green after the lib.rs/ops/init doc edits (AC5).
- **Doc build:** `cargo doc -p grove-core --no-deps` builds without warnings,
  confirming the `//!`/`///` docs render and intra-doc links (if any) resolve.
- **Library smoke test (AC3):** from `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/`,
  with `GROVE_REGISTRY` pointing at the repo's `registry/` (set in `main.rs` or the
  run command), `cargo run` compiles against `grove-core` by path and prints
  non-empty `ops::symbols(...)` output over the Rust fixture plus the
  `provision_project(..., dry_run=true)` narration (return is `Ok(Vec::new())` on
  dry-run — the evidence is the printed detected-languages output, not a non-empty
  vec). Exact commands, the resolved `GROVE_REGISTRY` path, and observed output
  captured in PROGRESS.
- **Clap-free re-confirmation (AC4):** `cargo tree` in the smoke crate shows no
  `clap` dependency; the smoke crate's committed `Cargo.lock` is inspected for the
  same.
- **Provenance:** PROGRESS records the smoke crate's detachment via the empty
  `[workspace]` table so it is excluded from the published workspace.

## Acceptance Criteria

- [ ] `core/src/lib.rs` exposes a curated public surface: `ops` and
  `init::provision_project` public, with return types (`Symbol`, `Defect`,
  `CallSite`, `MapEntry`, `FileMap`, `SourceResult`) re-exported; internal helpers
  not leaked; a top-level `//!` crate doc present (AC1).
- [ ] Public `ops` fns and `init::provision_project` carry `///` docs covering
  what/params/return, sufficient to render usefully on docs.rs (AC2).
- [ ] A standalone crate outside the workspace depends on `grove-core` by path and
  compiles + runs `ops::symbols(...)` (Rust fixture, grammar resolved via
  `GROVE_REGISTRY` → repo `registry/`) and `init::provision_project(...)` against a
  sample tree; exact commands, resolved registry path, and output documented in
  PROGRESS (AC3).
- [ ] `cargo tree` and the smoke crate's **committed** `Cargo.lock` confirm no
  `clap` in the `grove-core` consumer graph (AC4).
- [ ] `cargo test --release --locked` and `cargo clippy -- -D warnings` pass at the
  workspace root (AC5).
- [ ] *(Nice-to-have)* `core/README.md` with the `analyze_project` snippet wired as
  `readme` in `core/Cargo.toml`, and `core/examples/analyze_project.rs`.

## Operational Impact

- **Materiality:** Material — this changes `grove-core`'s public API (additive
  re-exports + docs). Per the stack checklist, library-surface changes that alter
  what consumers can import are material even though they are non-breaking.
- **Version bump:** Not required *in this task*. The additions are semver-additive
  (a minor-level change for `grove-core`); the publish/version decision is owned by
  T06, which wires `readme` and finalizes crate metadata. The smoke crate is never
  published.
- **Regeneration:** None.
- **Backward compatibility:** Additive only — existing `grove_core::ops::*` and
  `grove_core::init::*` paths remain valid; new root re-exports are pure additions.
  No CLI behavior changes; `grove init` and all commands are untouched.
- **Distribution:** No end-user action required. The smoke crate is a verification
  artifact under `engineering/sprints/...`, detached from the workspace via an empty
  `[workspace]` table, so it is excluded from `cargo build`/publish of the workspace.
