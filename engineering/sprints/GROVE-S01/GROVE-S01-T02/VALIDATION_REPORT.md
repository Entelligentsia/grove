# VALIDATION REPORT — GROVE-S01-T02: Extract grove-core library; rewire CLI (standalone review)

**Verdict:** Approved

Every acceptance criterion was re-verified against the live codebase and toolchain output — not trusted from PROGRESS.md. The two prior revisions (REVISION 1: `main.rs` drops `engine`; REVISION 2: core `serde` carries `features=["derive"]`) are both present.

## Acceptance Criteria — pass/fail per item

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | `grove-core` package, edition 2021, in root workspace `members`; `core/src/lib.rs` exposes `pub mod engine/ops/registry/fetch/ingest` | **PASS** | Root `Cargo.toml` `members = ["core", "cli"]`; `core/Cargo.toml` `name = "grove-core"`, `edition = "2021"`; `lib.rs` lists all 5 `pub mod`s. |
| 2 | `engine.rs`, `ops.rs`, `registry.rs`, `fetch.rs`, `ingest.rs` physically moved `cli/src/` → `core/src/` | **PASS** | `ls core/src/` shows all 5 + `lib.rs`; per-module check confirms each is gone from `cli/src/` (now only `init/main/mcp`). |
| 3 | `cli/Cargo.toml` declares `grove-core = { path = "../core" }`; `main.rs`/`mcp.rs`/`init.rs` reference `grove_core::…`; engine logic stays out of `main`/`mcp` | **PASS** | `cli/Cargo.toml` has the path dep; `main.rs:6 use grove_core::{ops, registry, fetch, ingest}`, `mcp.rs:14 use grove_core::{ops, registry}`, `init.rs:16 use grove_core::{fetch, registry}`. No `engine::` refs in `main.rs`. |
| 4 | `cargo tree -p grove-core` shows no `clap` | **PASS** | `cargo tree -p grove-core \| grep -i clap` → empty (clap-free). |
| 5 | `core/Cargo.toml` carries only core's deps (`tree-sitter`, `streaming-iterator`, `serde`, `serde_json`, `ignore`, `anyhow`, `sha2`, `dirs`, `ureq`); `clap` only in `cli/Cargo.toml` | **PASS** | `core/Cargo.toml` `[dependencies]` is exactly those 9 (serde with `features=["derive"]`); `cli/Cargo.toml` retains `clap`/`serde`/`serde_json`/`ignore`/`anyhow` + `grove-core`, drops the core-only deps. |
| 6 | build/test `--release --locked --workspace` green; `clippy --workspace -- -D warnings` clean; binary still `grove`; dev-stub counts unchanged | **PASS** | `cargo build --workspace` Finished; `cargo test --workspace` → 134 tests pass (32 cli + 17 integ + 85 core, 0 failed); `cargo clippy --workspace -- -D warnings` exit 0, no warnings; `grove --version` → `grove 0.1.11`; `GROVE_REGISTRY=registry grove languages` → 3 (dev-stub anchor), default `grove languages` → 21. |

## Regression / boundary checks

- **Test suite:** full workspace suite green including the `tests/cli.rs` integration suite (17 spawn-the-binary tests) and core's 85 unit tests — no regressions from the crate split.
- **Binary identity:** released artifact still named `grove`, version `0.1.11` — backward-compat bar holds (CLI/MCP/npm/Homebrew unaffected).
- **Dev-stub anchor:** 3-language dev-stub count unchanged; 21 default languages unchanged — registry behaviour preserved by the mechanical move.
- **Clap-free core:** independently confirmed via `cargo tree`, satisfying the sprint's gating backward-compat bar.

## Notes

- This task is sprint Step 2 (extract `grove-core`). The `init.rs` provisioning split (Step 3) and the library smoke-test bar are explicitly deferred to T03 per the task prompt — not in scope here and correctly not validated against.

All 6 acceptance criteria are met with reproduced evidence. Task is validated.
