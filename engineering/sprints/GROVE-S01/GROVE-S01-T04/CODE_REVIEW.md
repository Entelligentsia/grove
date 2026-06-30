# CODE REVIEW — GROVE-S01-T04: Curate public API, docs, and standalone library smoke test

🌿 *grove Supervisor* — (standalone review)

**Task:** GROVE-S01-T04

---

**Verdict:** Approved

---

## Review Summary

The implementation curates grove-core's public surface exactly as planned: a
docs.rs-quality crate-level `//!` tour, curated root `pub use` re-exports, and
expanded `///` what/params/return docs on the `ops` public functions. A
standalone smoke crate outside the workspace proves the API is consumable by a
foreign crate and clap-free. I independently reproduced every piece of test
evidence in PROGRESS.md — the 87+1 test result, clippy cleanliness, doc build,
the smoke run output (4 symbols + provision narration), and the absence of
`clap` in the smoke dependency graph — and all of it matches. No issues found.

## Checklist Results

| Item | Result | Notes |
|---|---|---|
| Re-export targets all `pub` (Symbol, Defect, CallSite, MapEntry, FileMap, SourceResult, provision_project) | ✓ | Verified via grep in engine.rs/ops.rs/init.rs — all `pub struct`/`pub fn`. |
| Internal helpers not leaked (Loaded, CapturedQuery, Index, Sources, Spec, Catalog) | ✓ | All plain `struct` (no `pub`) — not reachable from root re-exports. |
| `pub mod` declarations preserved (additive, non-breaking) | ✓ | All six modules still `pub`; cli imports registry/fetch/ingest directly. |
| Crate `//!` doc present + compiled `no_run` example | ✓ | Doctest at lib.rs:28 compiles (verified: 1 doctest passed). |
| `///` docs expanded to what/params/return on ops fns | ✓ | outline/symbols/source/check/definition all expanded; provision_project already adequate. |
| Smoke crate detached via empty `[workspace]` table | ✓ | Not in `cargo metadata` workspace members; own Cargo.lock. |
| `GROVE_REGISTRY` set to repo registry/ (AC3 reproducibility) | ✓ | 5th-ancestor path math correct; short-circuits registry_root() before cache cascade. |
| No `clap` in smoke dependency graph (AC4) | ✓ | `grep clap Cargo.lock` → 0; `cargo tree \| grep clap` → none. |
| `cargo test --release --locked -p grove-core` (AC5) | ✓ | 87 passed + 1 doctest, reproduced independently. |
| `cargo clippy -D warnings -p grove-core` (AC5) | ✓ | Clean, no warnings. |
| `cargo doc -p grove-core --no-deps` | ✓ | Builds, no warnings, intra-doc links resolve. |
| Smoke `cargo run` reproduces PROGRESS output (AC3) | ✓ | 4 defs (Point/new/distance/perimeter) + `detected rust 1 files` narration, `smoke OK`. |
| Path handling uses `Path::join`/`ancestors` (no string concat) | ✓ | `repo_registry()` uses `ancestors().nth(5)` + `join`. |
| No `unsafe` introduced | ✓ | Doc/re-export changes only. |
| Cargo.lock committed-eligible (durable AC4 provenance) | ✓ | Exists at smoke/Cargo.lock; smoke/.gitignore excludes only /target. |
| PROGRESS evidence authentic | ✓ | Every captured output block matches independent reproduction exactly. |

## Issues Found

None.

---

## If Approved

### Advisory Notes

1. **Stray root `.gitignore` change (non-T04).** The working tree has an
   uncommitted modification to the *root* `.gitignore` adding
   `.pi/pi-claude-compat/`. This is not a T04 file (T04 owns `smoke/.gitignore`,
   the smoke crate's own ignore). The commit choreography stages from the
   implementation summary's `files_changed` provenance, so it should not ride
   along — but confirm at commit time that the root `.gitignore` is not swept in.

2. **Working-tree has unrelated deletions.** `cli/src/{engine,fetch,ingest,ops,registry}.rs`
   and `src/*` are currently deleted in the working tree (from other in-progress
   work, not T04). A full-workspace `cargo test --release --locked` would fail
   today for that reason. I verified AC5 at the `grove-core` crate level
   (`-p grove-core`), which passes cleanly; T04's changes are not the cause and
   the PROGRESS evidence (showing both `grove-core` and `grove`/cli checking) was
   captured before those unrelated deletions landed.

3. **Nice-to-have deferred.** `core/README.md` + `core/examples/analyze_project.rs`
   were explicitly deferred to T06's publish-metadata pass. `core/Cargo.toml`
   already wires `readme = "../README.md"`. Acceptable per the approved plan.

4. **`std::env::set_var` in smoke `main`.** The smoke crate calls
   `std::env::set_var("GROVE_REGISTRY", …)` before invoking `ops::symbols`. This
   is correct for a standalone binary (no threading), and matches the plan's
   GROVE_REGISTRY reproducibility design. No change needed; noted only because
   `set_var` is unsafe in a multithreaded context (not applicable here).

5. **Stack-checklist writeback.** The smoke-crate pattern (empty `[workspace]`
   table to detach + `GROVE_REGISTRY` for reproducible grammar resolution in
   out-of-workspace test crates) is a reusable technique worth recording. Added
   a note to `engineering/architecture/stack-checklist.md` § Testing.