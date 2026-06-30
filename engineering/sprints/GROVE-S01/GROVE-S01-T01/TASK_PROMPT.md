# GROVE-S01-T01: Establish Cargo workspace; relocate crate into cli/

**Sprint:** GROVE-S01
**Estimate:** M
**Pipeline:** default

---

## Objective

Convert the binary-only repository into a virtual Cargo workspace as the
foundation for the core/CLI split, without changing any behavior. After this task
the repo root is a workspace, all current source lives under `cli/` as package
`grove`, and the build/test/binary all behave exactly as before.

## Acceptance Criteria

1. A root `Cargo.toml` is a **virtual manifest** — `[workspace]` with `members = ["cli"]`
   and **no `[package]` section**. (Add `"core"` to members in T02.)
2. All current `src/*.rs`, `tests/cli.rs`, and the `[dependencies]` + `[[bin]]` +
   `[profile.release]` move into `cli/` as package `grove` (`cli/Cargo.toml`,
   `cli/src/`, `cli/tests/`).
3. A single shared `Cargo.lock` remains at the repository root.
4. `cargo build --release --locked --workspace` succeeds and produces
   `target/release/grove` (binary name unchanged).
5. `cargo test --release --locked --workspace` is green — including `tests/cli.rs`
   and the dev-stub (`GROVE_REGISTRY=registry`) 3-language counts.
6. `cargo clippy --workspace -- -D warnings` is clean; files end with a newline.

## Context

First step of GitHub issue #50. The current crate is a single `[[bin]]` at the
repo root (`Cargo.toml` v0.1.11, edition 2021, name `grove`). This task is a pure
relocation — no module boundaries change yet; `core/` is created in T02. Keep
edition at 2021 and do NOT path-depend on tree-sitter workspace crates.

## Artifacts Involved

- New: root `Cargo.toml` (virtual workspace).
- Moved: `src/` → `cli/src/`, `tests/` → `cli/tests/`, package manifest → `cli/Cargo.toml`.
- Verify: `registry/` dev stub still resolves (path is relative to crate — confirm
  `GROVE_REGISTRY=registry` and the dev-tree fallback still find it from `cli/`).

## Operational Impact

- **Version bump:** not required (no release in this task).
- **Regeneration:** none.
- **Backward compat:** binary name `grove` and `target/release/grove` path
  unchanged — zero end-user impact.
