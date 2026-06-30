# Sprint Requirements — GROVE-S01

**Captured:** 2026-06-30
**Source:** sprint-intake interview
**Tracks:** [Entelligentsia/grove#50](https://github.com/Entelligentsia/grove/issues/50) — *Refactor grove into a Cargo Workspace (grove-core library + grove CLI)*

---

## Goals

1. Other Rust codebases can depend on a `grove-core` library and call Grove's
   AST operations natively (`grove_core::ops`, `grove_core::init::provision_project`)
   without pulling in CLI dependencies (`clap`, `serde_json`).
2. The repository is restructured as a Cargo workspace (`core/` + `cli/`) with the
   shipped binary still named `grove` — zero impact for CLI / MCP / npm / Homebrew users.
3. `grove-core` (and `grove`) are published to crates.io so library consumers can
   depend on a released version rather than a git path.

## In Scope

### Step 1 — Workspace root [must-have]
Convert the repo to a virtual Cargo workspace; relocate the existing crate source
into a `cli/` subdirectory as the starting point.

**Acceptance criteria:**
- A virtual workspace `Cargo.toml` exists at the repo root (no `[package]`, lists
  `core` and `cli` members).
- A single shared `Cargo.lock` at the root.
- `cargo build --release --workspace` succeeds.

### Step 2 — Extract `grove-core` [must-have]
Create `core/` (package `grove-core`) holding the engine modules with a public
library API.

**Acceptance criteria:**
- `core/src/lib.rs` exposes `engine`, `ops`, `registry`, `fetch`, `ingest` as
  `pub mod`s with a public, documented surface for `ops` and `init::provision_project`.
- `engine.rs`, `ops.rs`, `registry.rs`, `fetch.rs`, `ingest.rs` live under `core/src/`.
- `grove-core`'s dependency tree contains **no `clap`** (verified via
  `cargo tree -p grove-core`); `serde_json` removed from `grove-core` if no longer
  required by its public API.

### Step 3 — Split `init.rs` [must-have]
Separate provisioning (core) from agent-harness setup (CLI).

**Acceptance criteria:**
- `core/src/init.rs` exposes `pub fn provision_project(root: &Path, dry_run: bool) -> Result<Vec<String>>`
  that scans extensions, fetches missing grammars to the OS cache, and writes `grove.lock` —
  and does **not** write `.mcp.json` or `CLAUDE.md`.
- `cli/src/init.rs` retains the agent-harness logic (the `Target` enum `Mcp`/`Skill`,
  `.mcp.json` + `CLAUDE.md` generation) and calls `grove_core::init::provision_project()`
  first.
- `grove init --as mcp|skill|both [--dry-run]` behaves identically to today (same files
  written, same dry-run output shape).

### Step 4 — Refactor the CLI (`grove`) [must-have]
The `cli/` package keeps the canonical name `grove` and consumes the core API.

**Acceptance criteria:**
- `cli/Cargo.toml` declares package name `grove` and depends on
  `grove-core = { path = "../core" }`.
- `cli/src/main.rs` and `cli/src/mcp.rs` reference `grove_core::ops::…` (etc.)
  rather than `crate::ops::…`; engine logic stays out of `main`/`mcp`.
- `cargo build --release` produces a single binary named `grove`.

### Step 5 — Update CI & release tooling [must-have]
Make the build/release pipeline workspace-aware.

**Acceptance criteria:**
- `.github/workflows/release.yml` builds the workspace and produces the `grove`
  binary for all 5 target platforms (e.g. `cargo build --release -p grove`).
- `scripts/bump-version.sh` bumps the version in both `core/Cargo.toml` and
  `cli/Cargo.toml` (and `Cargo.lock` + `dist/npm/package.json`) so they stay in sync.
- `dist/npm` and Homebrew (`dist/homebrew/update-formula.sh`) release scripts still
  resolve and package the built `grove` binary correctly.
- A CI test job runs `cargo test --release --locked --workspace` green.

### crates.io publication [must-have]
Both packages are publishable to crates.io.

**Acceptance criteria:**
- `core/Cargo.toml` and `cli/Cargo.toml` carry required publish metadata
  (`description`, `license`, `repository`, `readme`, keywords/categories).
- `cargo publish --dry-run -p grove-core` and `-p grove` succeed.
- `grove` depends on `grove-core` by version (or path+version) so the published
  CLI resolves the published library.
- The crate name(s) are confirmed available/owned on crates.io (see Risks).

## Backward-Compatibility Acceptance Bar (gating for "done")

All four must hold:
- **`cargo test` green:** `cargo test --release --locked --workspace` passes,
  including the `tests/cli.rs` integration suite and the tokio def-count regression
  anchor; `GROVE_REGISTRY=registry` dev-stub counts (3-language) unchanged.
- **Binary still `grove`:** released artifact is named `grove`; npm
  `@entelligentsia/grove` and the Homebrew formula install unchanged.
- **clap-free core:** `grove-core` compiles with no `clap` in its dependency tree
  (`cargo tree -p grove-core` shows none).
- **Library smoke test:** a standalone throwaway crate depending on `grove-core`
  can call `ops::symbols(...)` and `init::provision_project(...)` and compile/run,
  proving the public API is usable.

## Out of Scope

- Splitting `core/` further into sub-crates (e.g. a separate `grove-engine`).
- Any change to the grammar registry format, `tags.scm`, or node classification
  behavior — language-agnostic classification via registry `tags.scm` is preserved
  unchanged (confirmed in issue thread).
- New library features beyond exposing the existing operations as a public API.
- Changing the MCP tool surface, CLI verbs, or their output formats.
- async/runtime changes to the engine.

## Nice-to-Have *(attempt if must-haves complete)*

- A short `core/README.md` (crate-level docs) with the issue's `analyze_project`
  example as a usage snippet.
- A minimal `examples/` program in `grove-core` demonstrating `provision_project` +
  `ops::symbols`.
- Doc comments (`///`) on the public `ops` / `init` surface sufficient for `docs.rs`.

## Constraints

- **Zero end-user impact:** published binary name (`grove`), npm package
  (`@entelligentsia/grove`), and Homebrew install path must not change.
- **Toolchain:** cargo 1.87; do NOT path-depend on tree-sitter workspace crates
  (edition 2024 / rust 1.90) — use crates.io versions. Crate edition stays 2021.
- **Warning-clean:** `cargo build` and `cargo clippy -- -D warnings` must pass
  across the workspace.
- **Single shared lockfile** at the workspace root; versions of `grove-core` and
  `grove` kept in lockstep.
- **Conventions:** files end with a newline; conventional-commit style; no
  `Co-Authored-By` / agent attribution on commits.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| crates.io name `grove` and/or `grove-core` already taken | Medium | Check availability early; if taken, fall back to an owned/namespaced name and surface the decision before publishing. Publication is the last gate — code restructure is independent of it. |
| Hidden `crate::`-relative paths / module visibility break on the move | High | Move incrementally, compile after each step; lean on grove's own tools to find `crate::ops`/`crate::engine` references before refactoring. |
| `serde_json` turns out to be required by the core public API (e.g. serialized return types) | Medium | If unavoidable, keep `serde`/`serde_json` in core but ensure `clap` is still excluded; the hard bar is clap-free, not serde-free. |
| Release workflow / npm / Homebrew scripts assume a single-package layout (paths to `target/release/grove`, `Cargo.toml` version locations) | Medium | Audit `release.yml`, `dist/npm`, `dist/homebrew/update-formula.sh`, `scripts/bump-version.sh` for hardcoded paths; verify with a dry-run before tagging. |
| Workspace build changes the `target/` layout used by `scripts/setup-local-test.sh` | Low | Re-verify the local-test path (`target/release/grove` is unchanged for a virtual workspace) after the move. |

## Carry-Over

None — GROVE-S01 is the first Forge sprint for this project.
