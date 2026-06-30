# PLAN — GROVE-S01-T02: Extract grove-core library; rewire CLI to consume it

🌱 *grove Engineer*

**Task:** GROVE-S01-T02
**Sprint:** GROVE-S01
**Estimate:** L

---

## Objective

Carve the engine out of the `grove` binary crate into a new `grove-core` library
crate and make the CLI a consumer of it. Five modules — `engine`, `ops`,
`registry`, `fetch`, `ingest` — move from `cli/src/` to `core/src/` behind a public
`lib.rs` API, while `main.rs`, `mcp.rs`, and `init.rs` stay in `cli/` and reference
`grove_core::…` instead of `crate::…`. The end state: `grove-core` is clap-free and
reusable by other Rust codebases, the `grove` binary and its CLI/MCP surface are
byte-for-byte unchanged, and `cargo … --workspace` is green and clippy-clean. This
is the heart of issue #50.

## Approach

T01 already produced the workspace (`members = ["cli"]`) with the crate relocated
under `cli/`. This task adds the second member and splits the source.

1. **Scaffold `core/`** — create `core/Cargo.toml` (package `grove-core`, edition
   2021) and `core/src/lib.rs` declaring `pub mod engine; pub mod ops; pub mod
   registry; pub mod fetch; pub mod ingest;`. Add `core` to the root workspace
   `members`. The `serde` dependency in `core/Cargo.toml` **must** carry
   `features = ["derive"]` — the moved modules use `#[derive(Serialize)]` /
   `#[derive(Deserialize)]` (e.g. `registry.rs` manifest structs, `ops.rs` output
   structs); omitting the feature breaks the build.
2. **Move modules incrementally** — relocate `engine.rs`, `ops.rs`, `registry.rs`,
   `fetch.rs`, `ingest.rs` from `cli/src/` to `core/src/` (via `git mv` to preserve
   history). These five modules reference each other only through `crate::engine`,
   `crate::registry`, `crate::fetch` — all of which **remain intra-crate** once they
   co-locate in `grove-core`, so their internal `crate::` paths need **no change**
   (research confirmed: every `crate::` ref inside the moved set targets another
   moved module). Compile core after wiring to catch hidden visibility breaks.
3. **Rewire the CLI consumers** — the only cross-crate edits:
   - `cli/src/main.rs`: drop `mod engine; mod fetch; mod ingest; mod ops; mod
     registry;` (keep `mod init; mod mcp;`) and add a single
     `use grove_core::{ops, registry, fetch, ingest};` so the existing
     `ops::…`, `fetch::run`, `ingest::run`, `registry::…` call sites resolve to the
     library. **`engine` is deliberately excluded from this import group:** `main.rs`
     contains zero `engine::` path references (only doc-comments and the removed `mod
     engine;`), and `engine` is consumed internally by `ops` inside `grove-core`
     (`crate::engine`). Importing it would be a dead `unused_imports` warning that
     `cargo clippy --workspace -- -D warnings` (AC#7) promotes to a hard error.
   - `cli/src/mcp.rs`: `use crate::{ops, registry};` → `use grove_core::{ops, registry};`.
   - `cli/src/init.rs`: `use crate::{fetch, registry};` → `use grove_core::{fetch, registry};`.
4. **Partition dependencies** — give `core/Cargo.toml` the engine's full dep set;
   add `grove-core = { path = "../core" }` to `cli/Cargo.toml` and trim it to the
   deps the CLI still references directly. Engine logic stays out of `main`/`mcp`.
5. **Verify** — workspace build/test/clippy green, `cargo tree -p grove-core` shows
   no `clap`, binary name still `grove`, dev-stub counts unchanged.

No engine logic is rewritten; this is a pure mechanical extraction + path rewire.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `Cargo.toml` (root) | `members = ["cli"]` → `["core", "cli"]` | Register the new library crate in the workspace. |
| `core/Cargo.toml` | **New** — package `grove-core`, edition 2021, engine deps | Library manifest; carries `tree-sitter`, `streaming-iterator`, `serde` **with `features = ["derive"]`** (moved modules derive `Serialize`/`Deserialize` — see `registry.rs`/`ops.rs`), `serde_json`, `ignore`, `anyhow`, `sha2`, `dirs`, `ureq` (no `clap`). |
| `core/src/lib.rs` | **New** — `pub mod engine/ops/registry/fetch/ingest;` | Public API surface so consumers can `use grove_core::ops;`. |
| `core/src/engine.rs` | **Moved** from `cli/src/engine.rs` | Core AST engine. Internal `crate::registry` paths stay valid. |
| `core/src/ops.rs` | **Moved** from `cli/src/ops.rs` | Operations layer. `crate::engine`/`crate::registry` stay valid. |
| `core/src/registry.rs` | **Moved** from `cli/src/registry.rs` | Grammar registry (no `crate::` refs). |
| `core/src/fetch.rs` | **Moved** from `cli/src/fetch.rs` | Grammar fetch. `crate::registry` stays valid. |
| `core/src/ingest.rs` | **Moved** from `cli/src/ingest.rs` | Source ingest. `crate::{fetch, registry}` stay valid. |
| `cli/Cargo.toml` | Add `grove-core` path dep; drop core-only deps (`tree-sitter`, `streaming-iterator`, `sha2`, `dirs`, `ureq`); keep `clap`, `anyhow`, `serde`, `serde_json`, `ignore` | CLI consumes the library; retains only the crates `main`/`mcp`/`init` use directly. `clap` lives **only** here. |
| `cli/src/main.rs` | Remove 5 `mod` decls; add `use grove_core::{ops, registry, fetch, ingest};` (no `engine` — unused in `main.rs`, would break clippy `-D warnings`) | Resolve existing bare-module call sites to the library. Keep `mod init; mod mcp;`. |
| `cli/src/mcp.rs` | `use crate::{ops, registry};` → `use grove_core::{ops, registry};` | MCP server consumes the library. |
| `cli/src/init.rs` | `use crate::{fetch, registry};` → `use grove_core::{fetch, registry};` | `init.rs` stays in CLI (provisioning split deferred to T03) but now sources `fetch`/`registry` from core. |

## Plugin Impact Assessment

- **Version bump required?** No — per the task prompt's Operational Impact, the
  CLI/MCP surface and binary name are unchanged; this is an internal crate split.
  Publishing `grove-core` to crates.io is a later sprint step, not this task.
- **Migration entry required?** No — no `.forge/store/` or config schema change.
- **Security scan required?** No — no `.forge/` tooling change; Rust source only.
- **Schema change?** No.

## Data Model Changes

None. No entity, store, or config schema is touched. The only structural change is
the Cargo workspace topology (one binary crate → binary crate + library crate) and
the Rust module ownership boundary (`crate::` → `grove_core::` at the three CLI
call sites). Public function signatures and behaviour are preserved verbatim.

## Testing Strategy

- **Workspace build:** `cargo build --release --locked --workspace` — green.
- **Workspace tests:** `cargo test --release --locked --workspace` — all existing
  unit tests (inline `#[cfg(test)]` in the moved modules) and `cli/tests/cli.rs`
  smoke tests pass unchanged.
- **Clap-free bar (AC#4):** `cargo tree -p grove-core` shows no `clap` anywhere in
  its dependency subtree (`cargo tree -p grove-core | grep -i clap` returns nothing).
  `serde`/`serde_json` remaining in core is acceptable.
- **Lint:** `cargo clippy --workspace -- -D warnings` — clean.
- **Binary identity:** built binary is still named `grove`; `grove --help` and a
  representative `grove outline`/`grove serve` invocation behave identically.
- **Dev-stub counts unchanged:** the registry dev-stub grammar count reported by the
  CLI is identical before and after the move.
- **Incremental compile:** build `core` alone first, then the workspace, to surface
  hidden `crate::`-relative path or visibility breaks early (the High risk called
  out in the requirements).

## Acceptance Criteria

- [ ] `core/` package `grove-core` exists, edition 2021, added to root workspace
      `members`; `core/src/lib.rs` exposes `pub mod engine; pub mod ops; pub mod
      registry; pub mod fetch; pub mod ingest;`.
- [ ] `engine.rs`, `ops.rs`, `registry.rs`, `fetch.rs`, `ingest.rs` physically moved
      from `cli/src/` to `core/src/`.
- [ ] `cli/Cargo.toml` declares `grove-core = { path = "../core" }`; `main.rs`,
      `mcp.rs`, `init.rs` reference `grove_core::…` instead of `crate::…`. Engine
      logic stays out of `main`/`mcp`.
- [ ] `cargo tree -p grove-core` shows no `clap` in its dependency tree.
- [ ] `core/Cargo.toml` carries only the deps core uses (`tree-sitter`,
      `streaming-iterator`, `serde` with `features = ["derive"]`, `serde_json`,
      `ignore`, `anyhow`, `sha2`, `dirs`, `ureq`); `clap` lives only in
      `cli/Cargo.toml`.
- [ ] `cli/src/main.rs`'s `grove_core` import group omits `engine` (unused there),
      keeping `cargo clippy --workspace -- -D warnings` clean.
- [ ] `cargo build --release --locked --workspace` and
      `cargo test --release --locked --workspace` green.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] Binary still named `grove`; dev-stub counts unchanged.

## Operational Impact

- **Distribution:** No end-user impact. Binary name, CLI flags, and MCP protocol
  surface are unchanged; no `/forge:update`-style action required of users.
- **Backwards compatibility:** Fully preserved. This is an internal refactor; the
  `grove` binary behaves identically.
- **Version bump:** Not required (internal crate split; no published-surface change
  in this task).
- **Regeneration:** None.
- **Material change?** Yes — structural Cargo/workspace change — but version-neutral
  per the task's stated operational impact.
- **Follow-on:** `init.rs`'s provisioning split and the `crate::`→`grove_core::`
  cleanup beyond these three files land in T03; crates.io publication is a later step.
