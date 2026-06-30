# Architect Approval — GROVE-S01-T02

**Verdict:** Approved

## Scope

Extract the `grove-core` library crate (engine, ops, registry, fetch, ingest) from the `grove` binary and rewire the CLI to consume it via `grove_core::`. Pure mechanical extraction — no engine logic rewritten.

## Architectural Review

Independently re-verified against the live codebase and toolchain (not trusted from prior summaries):

- **Two-crate lib+binary split is sound.** `core/` (`grove-core`, edition 2021) holds the reusable structural code-intelligence core; `cli/` (`grove`) is a thin clap-driven shell. This matches the `stack.md` posture: the library is the engine behind both the CLI and the MCP server, and the split positions future consumers to depend on `grove-core` directly.
- **Workspace wired correctly.** Root `Cargo.toml` `members = ["core", "cli"]`, resolver 2. `core/src/lib.rs` exposes `pub mod engine/ops/registry/fetch/ingest`. The five modules physically live in `core/src/`; `cli/src/` now contains only `init/main/mcp`.
- **AC#4 (clap-free core) holds.** `cargo tree -p grove-core | grep -i clap` returns nothing. `clap` lives only in `cli/Cargo.toml`. The CLI parsing concern is correctly isolated to the binary.
- **Dep partitioning is clean.** `core/Cargo.toml` carries exactly the 9 engine deps with `serde` `features=["derive"]` (REVISION 2 — required by the moved modules' `Serialize`/`Deserialize` derives). `cli/Cargo.toml` retains only `clap/serde/serde_json/ignore/anyhow` plus the `grove-core` path dep; dropped deps (`tree-sitter`, `streaming-iterator`, `sha2`, `dirs`, `ureq`) are gone from the CLI.
- **REVISION 1 applied.** `main.rs` imports `use grove_core::{ops, registry, fetch, ingest}` — `engine` excluded, avoiding a clippy `unused_imports` under `-D warnings`. No `engine::` paths remain in `main.rs`.
- **Consumer rewiring correct.** `mcp.rs:14` → `use grove_core::{ops, registry}`; `init.rs:16` → `use grove_core::{fetch, registry}`. Intra-core `crate::` refs target only moved modules; no visibility bumps were needed.
- **Toolchain green.** `cargo clippy --workspace -- -D warnings` exits 0. Prior phases report 134 tests pass (32 cli + 17 integ + 85 core), binary still `grove 0.1.11`, dev-stub/default registry counts unchanged. Integration tests spawn the binary as a subprocess (`CARGO_BIN_EXE_grove`) and are unaffected by the move.

## Cross-Cutting Concerns

- **No security surface touched.** No network, auth, or filesystem-trust code was modified — only relocated.
- **No engine logic rewritten.** `git mv` preserved history; intra-core `crate::` refs stayed valid with no edits.
- **History preservation.** The move used `git mv`, so blame history across the relocated modules is retained.

## Operational Impact

- **Version bump:** not required. Both crates at `0.1.11`.
- **Binary identity:** unchanged — still `grove`. Distribution channels (GitHub Releases, npm, Homebrew, install script) unaffected.
- **Regeneration:** none. `Cargo.lock` regenerated with the new `grove-core` package entry and is checked in, so source builds remain deterministic.
- **Backward compat:** CLI/MCP surface and binary name unchanged.

## Follow-Up Items

- **T03 (deferred scope):** `init.rs` provisioning split and a library smoke test — correctly held back from T02 per the validation report.
- **Advisory (no action):** `cli/Cargo.toml` carries `serde` `features=["derive"]` that is technically unused in the CLI (only a `T: serde::Serialize` trait bound at `mcp.rs:329`). Harmless and matches the plan; may be trimmed in a future cleanup pass.
- **Future:** if external consumers want the library without the CLI, `grove-core` is now structured for independent publication.