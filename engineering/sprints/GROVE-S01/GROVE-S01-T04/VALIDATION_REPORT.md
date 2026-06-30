# VALIDATION_REPORT — GROVE-S01-T04: Curate public API, docs, and standalone library smoke test (standalone review)

**Verdict:** Approved

All five must-have acceptance criteria were validated independently against the as-written
plan. Each was reproduced from source/disk, not taken on trust from PROGRESS.

## Acceptance Criteria

### AC1 — Curated public surface ✓ PASS
- `core/src/lib.rs` keeps all six `pub mod` declarations (engine/init/ops/registry/fetch/ingest)
  — additive, non-breaking (cli imports registry/fetch/ingest directly).
- Curated `pub use` re-exports present and verified: `engine::{Defect, Symbol}`,
  `ops::{CallSite, FileMap, MapEntry, SourceResult}`, `init::provision_project`.
- Top-level `//!` crate doc present (overview + compiled `no_run` example).
- Named internal helpers (Loaded, CapturedQuery, Index, Sources, Spec, Catalog) NOT
  re-exported at root and not leaked. (`fetch::CatalogGrammar` is pub within the
  intentionally-public `fetch` module — not one of the internal helpers AC1 names.)

### AC2 — docs.rs-quality docs ✓ PASS
- Public `ops` fns and `init::provision_project` carry `///` docs.
- Evidence: `cargo doc -p grove-core --no-deps` builds with **no warnings**; the crate
  `no_run` example compiles as a doctest (1 passed).

### AC3 — Standalone out-of-workspace smoke crate ✓ PASS
- `smoke/Cargo.toml` has empty `[workspace]` table (detached) + `grove-core` path dep.
- `cargo run --locked` reproduced live:
  - `GROVE_REGISTRY` resolved to repo `registry/` (5th-ancestor path math — reproducible).
  - `ops::symbols` over the Rust fixture returned **4 definitions** (Point/new/distance/perimeter).
  - `init::provision_project(sample, dry_run=true)` detected rust (1 file), returned 0 actions.
  - Terminal `smoke OK`.

### AC4 — No clap in consumer graph ✓ PASS
- `grep -c clap smoke/Cargo.lock` → **0**.
- `cargo tree | grep -c clap` (smoke crate) → **0**.

### AC5 — Workspace gate green ✓ PASS
- `cargo test --release --locked` → **87 passed; 0 failed** + 1 doctest passed.
- `cargo clippy -- -D warnings` → clean (no warnings).

### Nice-to-have — `core/README.md` + `core/examples/` — N/A
- Deferred to T06 per the approved plan. Not a must-have; does not affect the verdict.

## Regression Check
- Full workspace test suite green (87 + doctest); clippy clean. No regressions observed.

The task satisfies every must-have acceptance criterion as written. Validated.
