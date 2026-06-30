# CODE REVIEW — GROVE-S01-T02: Extract grove-core library; rewire CLI to consume it

**(standalone review)**

## Verdict: **Approved**

The implementation faithfully executes the approved PLAN.md. Every acceptance
criterion was re-verified independently against the live codebase and toolchain
output — PROGRESS.md's claims were not taken on trust.

## Spec Compliance (verified independently)

| AC | Claim | Independent verification | Result |
|----|-------|---------------------------|--------|
| #1 | `core/` package `grove-core`, edition 2021, in root workspace | `core/Cargo.toml` reads `edition = "2021"`; root `Cargo.toml` `members = ["core", "cli"]` | ✅ |
| #2 | 5 modules physically moved to `core/src/` | `ls core/src/` → engine/ops/registry/fetch/ingest + lib.rs; `ls cli/src/` → only init/main/mcp | ✅ |
| #3 | `cli/Cargo.toml` declares `grove-core` path dep; consumers use `grove_core::` | `cli/Cargo.toml` has `grove-core = { path = "../core" }`; main.rs:6, mcp.rs:14, init.rs:16 all `use grove_core::{…}` | ✅ |
| #4 | `cargo tree -p grove-core` shows no `clap` | Ran `cargo tree -p grove-core \| grep -i clap` → empty | ✅ |
| #5 | `core/Cargo.toml` carries only core deps; `serde` has `features=["derive"]` | Confirmed dep list + `serde = { version = "1", features = ["derive"] }` (REVISION 2) | ✅ |
| #6 | `main.rs` import omits `engine` | `use grove_core::{ops, registry, fetch, ingest};` — no `engine`; `grep engine:: cli/src/main.rs` → empty (REVISION 1) | ✅ |
| #7 | `cargo build --release --locked --workspace` green | Re-ran → `Finished release` | ✅ |
| #8 | `cargo test … --workspace` green; `clippy -D warnings` clean | Re-ran: 32 cli + 17 integ + 85 core = 134 passed, 0 failed; clippy clean | ✅ |
| #9 | Binary still `grove`; dev-stub/lang counts unchanged | `./target/release/grove --version` → `grove 0.1.11`; `grove languages` emits the grammar table; registry tests (incl. dev-stub) pass | ✅ |

## Correctness

- **REVISION 1 (engine exclusion):** Correctly applied. `main.rs` imports
  `{ops, registry, fetch, ingest}` with no `engine`; a full `grep engine::`
  against `main.rs` returns nothing. `engine` is consumed internally by `ops`
  inside `grove-core` (`crate::engine`), so excluding it from the CLI import is
  the right call and keeps `clippy -D warnings` honest.
- **REVISION 2 (serde derive):** Correctly applied. `core/Cargo.toml` carries
  `features = ["derive"]`; the moved modules derive `Serialize`/`Deserialize`
  (e.g. registry manifest structs, ops output structs), so this is load-bearing.
- **Intra-core `crate::` refs:** Audited every `crate::` reference in `core/src/`
  — they target only `engine`, `registry`, `fetch` (all co-located in
  `grove-core`). No reference targets `init` or `mcp` (which stayed in the CLI).
  The verbatim-move strategy holds; no path edits were needed inside the moved
  set. The one `crate::Scanner` hit in `ops.rs` is a string literal inside a test
  assertion, not a path — harmless.
- **CLI rewires:** `main.rs` dropped the 5 `mod` decls, kept `mod init; mod mcp;`,
  and rewired call sites (`init::run`, `init::Target`, `mcp::serve`) resolve to
  the local CLI modules. `mcp.rs` and `init.rs` each flipped `use crate::{…}` →
  `use grove_core::{…}`. No leftover `use crate::` lines remain in `cli/src/`.
- **Dep partitioning:** `cli/Cargo.toml` retains `clap`, `serde`, `serde_json`,
  `ignore`, `anyhow` (+ `grove-core`); dropped `tree-sitter`,
  `streaming-iterator`, `sha2`, `dirs`, `ureq`. A grep for the dropped crates in
  `cli/src/` returns nothing; every retained crate is actually used in the
  resident files. `clap` lives only in the CLI crate — confirmed clap-free core.
- **Cargo.lock:** Regenerated with a `grove-core` 0.1.11 entry; `--locked` builds
  succeed, so the lock is consistent with the manifests.
- **Integration tests:** `cli/tests/cli.rs` invokes
  `Command::new(env!("CARGO_BIN_EXE_grove"))` — it exercises the built binary as
  a subprocess, with no `crate::`/`grove_core::` refs. The move cannot affect it;
  all 17 pass.

## Security
No security surface touched. This is an internal structural refactor: no auth,
input parsing, or external I/O logic changed. The moved modules are byte-for-byte
relocated (git mv preserved), so their existing validation/sanitisation behaviour
is unchanged.

## Architecture & Conventions
- Two-crate lib+binary workspace (`grove-core` library + `grove` binary) is the
  sound shape for the planned T03+ provisioning split. `clap` correctly lives
  only in the binary; the core library stays CLI-agnostic.
- `core/src/lib.rs` exposes `pub mod engine/ops/registry/fetch/ingest` with
  crate-level docs explaining the clap-free design intent — good convention
  adherence.
- Manifests carry consistent metadata (version 0.1.11, license, repository,
  keywords/categories) across both crates.
- No engine logic rewritten; the refactor is purely mechanical, as planned.

## Testing
- 134 tests pass (re-run by reviewer): 32 CLI unit, 17 integration (subprocess),
  85 core unit. Counts match PROGRESS.md exactly.
- `cargo clippy --workspace -- -D warnings` clean.
- Test evidence in PROGRESS.md is authentic: the embedded output blocks are
  consistent with the re-run results (pass counts, clap-free tree, binary
  version).

## Advisory Notes (non-blocking)

1. **`cli/Cargo.toml` serde derive feature — technically unused in the CLI.** The
   CLI's only `serde` use is a trait bound `T: serde::Serialize` (`mcp.rs:329`);
   there is no `#[derive(Serialize/Deserialize)]` in `cli/src/`. The `derive`
   feature on the CLI's `serde` dep is therefore not strictly required. Keeping
   it is harmless, defensive, and matches the plan's explicit choice — but it
   could be trimmed in a future cleanup pass. No action required for this task.
2. **Working-tree staging state** shows a mix of staged renames/deletions and
   unstaged modifications. This is a concern for the commit phase, not the code
   review — the code on disk is correct and self-consistent.

## Conclusion
A clean, faithful, low-risk mechanical extraction. Both prior plan-review
revisions landed correctly. All acceptance criteria independently confirmed.
No security, architecture, or convention issues. **Approved.**