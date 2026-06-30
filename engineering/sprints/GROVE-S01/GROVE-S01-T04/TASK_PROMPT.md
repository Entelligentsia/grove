# GROVE-S01-T04: Curate public API, docs, and standalone library smoke test

**Sprint:** GROVE-S01
**Estimate:** M
**Pipeline:** default

---

## Objective

Turn the mechanically-exposed `grove-core` modules into a deliberate, documented
public API and **prove** it works from outside the workspace. This is the "huge win"
deliverable of issue #50 — a Rust consumer using Grove's AST operations natively —
and it satisfies the **library smoke test** backward-compat bar.

## Acceptance Criteria

1. `core/src/lib.rs` presents a curated public surface: `ops` and
   `init::provision_project` are public, along with the return types a consumer
   needs (`Symbol`, `Defect`, `CallSite`, `MapEntry`/`FileMap`, etc.). Internal-only
   helpers are not leaked unnecessarily. A top-level crate doc comment (`//!`)
   describes the library.
2. The public `ops` and `init` items carry `///` doc comments sufficient to render
   usefully on docs.rs (each public fn: what it does, params, return).
3. **Library smoke test:** a standalone crate *outside* the workspace (e.g. under
   `engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke/`, or a documented throwaway)
   depends on `grove-core` by path, and compiles + runs `ops::symbols(...)` and
   `init::provision_project(...)` successfully against a sample tree. Document the
   exact commands and observed output in the task's PROGRESS.
4. `grove-core`'s public API does not require the consumer to add `clap`
   (re-confirm `cargo tree`); the smoke crate's lockfile shows no `clap`.
5. Workspace build/test/clippy stay green.

## Nice-to-Have *(if must-haves complete)*

- `core/README.md` (crate-level) containing the issue's `analyze_project` snippet,
  wired as `readme` in `core/Cargo.toml` (feeds T06 publish metadata).
- A `core/examples/analyze_project.rs` example mirroring the snippet.

## Context

Depends on **T03** (`provision_project` exists in core). The smoke test is a
*verification artifact*, not shipped product code — keep it out of the published
crate (don't add it as a workspace member, or gate it clearly). It is the concrete
evidence for the "Library smoke test" gate in the requirements.

## Artifacts Involved

- Edited: `core/src/lib.rs` (curated re-exports + docs), `core/src/ops.rs`/`init.rs`
  (`///` docs on public items).
- New (verification): standalone smoke crate; optionally `core/README.md`,
  `core/examples/analyze_project.rs`.

## Operational Impact

- **Version bump:** not required.
- **Regeneration:** none.
- **Backward compat:** additive (docs + exports); no behavior change.
