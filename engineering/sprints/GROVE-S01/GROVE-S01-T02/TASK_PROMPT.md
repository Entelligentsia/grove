# GROVE-S01-T02: Extract grove-core library; rewire CLI to consume it

**Sprint:** GROVE-S01
**Estimate:** L
**Pipeline:** default

---

## Objective

Create the `grove-core` library crate holding Grove's AST engine and expose it as
a public API, so other Rust codebases can `use grove_core::ops;` without the CLI's
`clap` dependency. The `grove` CLI becomes a consumer of that library. This is the
heart of issue #50 and folds in the mechanical part of "Step 4" (the
`crate::` → `grove_core::` rewire is unavoidable for the workspace to compile).

## Acceptance Criteria

1. New `core/` package `grove-core` (added to root workspace `members`), edition
   2021, with `core/src/lib.rs` exposing `pub mod engine; pub mod ops; pub mod
   registry; pub mod fetch; pub mod ingest;`.
2. `engine.rs`, `ops.rs`, `registry.rs`, `fetch.rs`, `ingest.rs` physically move
   from `cli/src/` to `core/src/`.
3. `cli/Cargo.toml` declares `grove-core = { path = "../core" }`; `cli/src/main.rs`,
   `cli/src/mcp.rs` (and any remaining cli module) reference `grove_core::ops::…`
   etc. instead of `crate::ops::…`. Engine logic stays out of `main`/`mcp`.
4. **`cargo tree -p grove-core` shows no `clap`** anywhere in its dependency tree.
   (`serde`/`serde_json` stay in core — `ops::project` returns `serde_json::Value`
   and `registry.rs` is serde-heavy; that is acceptable, the bar is clap-free.)
5. `core/Cargo.toml` carries only the deps core actually uses (`tree-sitter`,
   `streaming-iterator`, `serde`, `serde_json`, `ignore`, `anyhow`, `sha2`, `dirs`,
   `ureq`); `clap` lives only in `cli/Cargo.toml`.
6. `cargo build/test --release --locked --workspace` green; `cargo clippy --workspace
   -- -D warnings` clean; binary still `grove`; dev-stub counts unchanged.

## Context

Depends on **T01** (workspace + `cli/`). `clap` is used only in `main.rs` and
`init.rs`; `init.rs` stays in `cli/` for now (its provisioning split happens in
**T03**). Move modules incrementally and compile after each to catch hidden
`crate::`-relative paths and visibility breaks (the High risk in the requirements);
use grove's own `symbols`/`callers` to find `crate::` references first.

## Artifacts Involved

- New: `core/Cargo.toml`, `core/src/lib.rs`.
- Moved: `engine.rs`, `ops.rs`, `registry.rs`, `fetch.rs`, `ingest.rs` → `core/src/`.
- Edited: `cli/Cargo.toml` (add grove-core dep, drop core-only deps), `cli/src/main.rs`,
  `cli/src/mcp.rs`, root `Cargo.toml` (add `core` member).

## Operational Impact

- **Version bump:** not required.
- **Regeneration:** none.
- **Backward compat:** CLI/MCP surface and binary name unchanged.
