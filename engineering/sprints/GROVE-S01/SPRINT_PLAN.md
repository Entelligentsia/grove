# Sprint Plan — GROVE-S01

**Refactor grove into a Cargo workspace (grove-core library + grove CLI)**

**Planned:** 2026-06-30
**Tracks:** [Entelligentsia/grove#50](https://github.com/Entelligentsia/grove/issues/50)
**Requirements:** [SPRINT_REQUIREMENTS.md](SPRINT_REQUIREMENTS.md)

---

## Approach

The migration is a sequential restructure where each step must keep the workspace
compiling and `cargo test --workspace` green. We decompose into six tasks that
mirror the issue's steps, with two adjustments grounded in the current code:

1. **`serde_json` stays in `grove-core`.** `ops::project` returns `serde_json::Value`
   (outline tiering) and `registry.rs` is serde-heavy. The hard bar is **clap-free**,
   not serde-free (per the requirements). `clap` is used only in `main.rs` and
   `init.rs` (the `Target` enum) — both stay CLI-side.
2. **Step 4's mechanical rewire folds into Step 2.** Moving the engine modules to
   `grove-core` forces `crate::ops::…` → `grove_core::ops::…` in `main.rs`/`mcp.rs`
   immediately (the crate won't compile otherwise). So T02 carries that rewire, and
   the dedicated "Step 4" task (T04) becomes the **public-API contract + library
   smoke test** — the actual "huge win" deliverable proof.

Each task lands on `main` only after passing the warning-clean build + full test
suite; the binary stays named `grove` and `target/release/grove` stays the artifact
path throughout (a virtual workspace doesn't change it).

## Tasks

| ID | Title | Est | Depends on | Step |
|---|---|---|---|---|
| GROVE-S01-T01 | Establish Cargo workspace; relocate crate into `cli/` | M | — | 1 |
| GROVE-S01-T02 | Extract `grove-core` library; rewire CLI to consume it | L | T01 | 2 (+4 mech.) |
| GROVE-S01-T03 | Split `init.rs` — `provision_project` (core) vs harness (CLI) | M | T02 | 3 |
| GROVE-S01-T04 | Curate public API, docs, and standalone library smoke test | M | T03 | 4 |
| GROVE-S01-T05 | Make CI & release tooling workspace-aware | M | T02 | 5 |
| GROVE-S01-T06 | crates.io publication readiness for `grove-core` + `grove` | M | T04, T05 | publish |

## Dependency graph & critical path

```
T01 ──► T02 ──► T03 ──► T04 ──────► T06
                 │                  ▲
                 └──► T05 ──────────┘
```

- **Critical path:** T01 → T02 → T03 → T04 → T06 (5 tasks, ~M+L+M+M+M).
- **T05** branches off T02 (the `core/`+`cli/` layout and dual `Cargo.toml`s exist
  after T02) and rejoins at T06. It can run in parallel with T03/T04.
- **Execution mode:** primarily **sequential** on the critical path; T05 is the one
  parallelizable branch.

## Per-task acceptance summary

- **T01** — root virtual `Cargo.toml` (members `cli`, no `[package]`), all current
  `src/` + `tests/` + deps under `cli/` (package `grove`), shared root `Cargo.lock`,
  `cargo build/test --release --locked --workspace` green, binary still `grove`.
- **T02** — `core/` package `grove-core` with `lib.rs` exposing
  `pub mod engine|ops|registry|fetch|ingest`; those files moved out of `cli/`;
  `cli` depends on `grove-core = { path = "../core" }`; `main.rs`/`mcp.rs` use
  `grove_core::…`; **`cargo tree -p grove-core` shows no `clap`**; workspace builds &
  tests green; clippy clean.
- **T03** — `grove_core::init::provision_project(root, dry_run) -> Result<Vec<String>>`
  (extension scan + grammar fetch + `grove.lock`, **no** `.mcp.json`/`CLAUDE.md`);
  `cli::init` keeps `Target`/harness writers and calls `provision_project` first;
  `grove init --as mcp|skill|both [--dry-run]` byte-identical behavior (files written
  + dry-run output) verified by existing `tests/cli.rs`.
- **T04** — `lib.rs` exports a curated, `///`-documented public surface (`ops`,
  `init::provision_project`, the `Symbol`/`Defect`/etc. return types); a throwaway
  crate depending on `grove-core` compiles and runs `ops::symbols(...)` +
  `init::provision_project(...)` (the library smoke test); `core/README.md` with the
  issue's `analyze_project` snippet [nice-to-have].
- **T05** — `release.yml` builds the workspace and still emits `grove` for all 5
  targets; `ci.yml` runs `cargo build/test --release --locked --workspace`;
  `bump-version.sh` bumps **both** `core/Cargo.toml` and `cli/Cargo.toml` (+ `Cargo.lock`
  + `dist/npm/package.json`) in lockstep; `setup-local-test.sh`, `dist/npm`, and
  `dist/homebrew/update-formula.sh` verified to still resolve `target/release/grove`.
- **T06** — publish metadata on both crates; `grove` depends on `grove-core` by
  version (path+version); `cargo publish --dry-run -p grove-core` and `-p grove`
  succeed; crate-name availability on crates.io confirmed (fallback name surfaced for
  decision if `grove`/`grove-core` is taken — **publication is the final gate**).

## Risk notes (from requirements)

- **High:** hidden `crate::`-relative paths breaking on the move → T02 moves
  incrementally and compiles after each module; use grove's own `symbols`/`callers`
  to find `crate::` references first.
- **Medium:** crates.io name availability → checked in T06, but it gates only
  publication, not the restructure.
- **Medium:** release/script hardcoded paths → audited in T05 with a release
  `--dry-run` before any tag.

## Out of scope (carried from requirements)

Further core sub-crate splits; any registry/`tags.scm`/classification change; new
library features beyond exposing existing ops; MCP/CLI surface or output changes;
async/runtime changes.
