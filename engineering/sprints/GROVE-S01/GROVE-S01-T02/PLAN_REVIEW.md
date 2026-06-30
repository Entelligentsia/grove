# PLAN REVIEW — GROVE-S01-T02: Extract grove-core library; rewire CLI to consume it

🌿 *grove Supervisor — Oracle*

**Task:** GROVE-S01-T02
**Phase:** review-plan (standalone review)

---

**Verdict:** Approved

---

## Review Summary

The revised plan correctly incorporates both items from the prior "Revision
Required" review: the unused `engine` import is dropped from `main.rs`'s
`grove_core` use group, and `core/Cargo.toml`'s `serde` dependency explicitly
carries `features = ["derive"]`. I independently re-verified every load-bearing
claim against the live codebase — including a visibility audit the prior review
did not document — and all hold. The plan is architecturally sound, correctly
scoped, and ready for implementation.

## Prior-Revision Verification

Both required revisions from the prior review are correctly applied **and
propagated consistently** across the three locations they touch (Approach prose,
Files-to-Modify table, Acceptance Criteria list):

1. **Drop `engine` from `main.rs` import (was DEFECT, blocking AC#7).**
   - Approach §3 now reads `use grove_core::{ops, registry, fetch, ingest};` with
     an explicit rationale ("`engine` is deliberately excluded … would break
     clippy `-D warnings`").
   - Files-to-Modify `main.rs` row matches: "(no `engine` — unused in `main.rs`)".
   - AC list adds an explicit clause: "import group omits `engine`".
   - **Independently re-verified:** `grep -n "engine::" cli/src/main.rs` returns
     **nothing**. The only `engine` tokens in `main.rs` are doc-comments
     (lines 1, 4) and the `mod engine;` decl (line 6) the plan removes. There is
     no crate-level `#![allow(unused_imports)]`. Dropping `engine` is correct.

2. **`core/Cargo.toml` serde must carry `features = ["derive"]` (was ADVISORY).**
   - Approach §1, the core/Cargo.toml table row, and the AC list now all
     explicitly note `serde` **with `features = ["derive"]`**.
   - **Independently re-verified:** `registry.rs` derives `Deserialize`
     (lines 19, 66, 73) and `ops.rs` derives `Serialize` (lines 164, 228, 400,
     415). The `derive` feature is required for these to compile.

## Independent Claim Verification (full re-audit)

I did not rely on the prior review's findings — I re-ran every check against the
actual source.

- **Intra-core `crate::` refs need no change — VERIFIED.** Every `crate::`
  reference in the five moved modules targets only another moved module:
  - `engine.rs` → `crate::registry` (×2) — moved ✓
  - `ops.rs` → `crate::engine`, `crate::registry` — moved ✓
    (the `crate::Scanner` on line 808 is a string literal inside a test
    assertion, not a path)
  - `registry.rs` → none
  - `fetch.rs` → `crate::registry`, `crate::registry::sha256` — moved ✓
  - `ingest.rs` → `crate::{fetch, registry}` — moved ✓
  None reference `crate::init` or `crate::mcp`. Modules move verbatim.

- **Cross-crate visibility — VERIFIED (new check, not in prior review).** This
  is the hidden risk the task prompt flags as "High": items accessed from `cli`
  across the crate boundary must be `pub`, but in a binary crate nothing needs
  `pub` to be crate-visible. I grepped every `pub` declaration in the moved set
  and cross-checked every call site in the three resident files:
  - `main.rs` calls `ops::{outline,project,rel,symbols,source,check,callers,map,
    parse_pos,definition_at,definition}`, `fetch::run`, `ingest::run`,
    `registry::{write_index,root,search_path,manifests,write_lock}` — **all `pub`** ✓
  - `mcp.rs` calls `registry::available`, `ops::{outline,project,symbols,source,
    check,callers,map,parse_pos,definition_at,definition}` — **all `pub`** ✓
  - `init.rs` calls `fetch::{run,catalog_grammars}`,
    `registry::{write_lock_for,manifests,cache_root}` — **all `pub`** ✓
  No item needs a visibility bump. The "move verbatim" claim holds end-to-end.

- **Core dep set complete and clap-free — VERIFIED.** Per-module dep audit:
  | dep | used by moved module(s) |
  |---|---|
  | tree-sitter | engine.rs |
  | streaming-iterator | engine.rs |
  | serde (derive) | registry.rs, ops.rs |
  | serde_json | ops, registry, fetch, ingest |
  | ignore | ops.rs (WalkBuilder) |
  | anyhow | all five |
  | sha2 | registry.rs, fetch.rs |
  | dirs | registry.rs |
  | ureq | fetch.rs |
  All nine assigned deps are actually referenced. `clap::` appears in **none** of
  the moved modules → AC#4 (`cargo tree -p grove-core` clap-free) is achievable.

- **CLI dep trim correct — VERIFIED.** Retained deps all used directly in
  resident files: `clap` (main, init), `anyhow` (all three), `serde`
  (mcp.rs:329 trait bound), `serde_json` (all three), `ignore` (init, mcp).
  Dropped deps all unused in resident files — the only "tree-sitter" hits in
  `main.rs`/`init.rs`/`mcp.rs` are doc-comment/description strings, not imports.
  `streaming-iterator`, `sha2`, `dirs`, `ureq` have zero hits in resident files.

- **Resident-file rewire points — VERIFIED.** `mcp.rs:14` is exactly
  `use crate::{ops, registry};` and `init.rs:16` is exactly
  `use crate::{fetch, registry};`. `main.rs` has no `use crate::` line — it uses
  `mod` declarations (lines 6–11), which the plan removes and replaces with the
  single `grove_core` import.

- **Integration tests unaffected — VERIFIED.** `cli/tests/cli.rs` invokes the
  built binary as a subprocess (`Command::new(env!("CARGO_BIN_EXE_grove"))`)
  with zero `crate::` references. The module relocation does not touch the test
  surface.

## Feasibility

The approach is realistic and minimal: scaffold `core/`, `git mv` five modules,
rewire three CLI call sites, partition deps. No engine logic is rewritten. The
"build core alone first, then workspace" incremental step directly mitigates the
High-risk hidden-break concern, which my visibility audit confirms is already
clear. Scope matches the task prompt's AC#1–6 exactly.

## Plugin Impact Assessment

- **Version bump declared correctly?** Yes — none required (internal crate
  split; no published-surface change).
- **Migration entry targets correct?** N/A — no `.forge/store/` or config schema
  change.
- **Security scan requirement acknowledged?** Yes — none required (no `.forge/`
  tooling change; Rust source only).

## Security

No security surface touched. No auth, input parsing, or network code is added —
`fetch` merely relocates. No new Markdown or hook code. No injection or
exfiltration risk.

## Architecture Alignment

The `core/` (library) + `cli/` (binary consumer) split matches the intended
two-crate workspace topology from issue #50 and is consistent with the
stack-checklist Rust guidance (edition 2021, `cargo clippy -- -D warnings`,
release build with `lto = "thin"` / `strip = true`). `pub mod` exposure is the
idiomatic library surface. `git mv` preserves history. Binary name (`grove`),
CLI flags, and MCP protocol surface are preserved verbatim. No schema changes.

## Testing Strategy

Adequate and matched to the stack-checklist:
- `cargo build --release --locked --workspace` — green.
- `cargo test --release --locked --workspace` — unit tests (inline in moved
  modules) + `cli/tests/cli.rs` smoke tests pass unchanged.
- `cargo tree -p grove-core | grep -i clap` returns nothing (AC#4 bar).
- `cargo clippy --workspace -- -D warnings` clean (AC#7 — now unblocked by the
  engine-import fix).
- Binary identity + dev-stub count stability.
- Incremental core-first compile surfaces hidden breaks early.

## Advisory Notes (non-blocking)

- **`engine` is `pub mod` in `lib.rs` but unused by `cli`.** This is correct per
  AC#1 (the spec mandates `pub mod engine;`) and harmless — `engine` is part of
  the library's intended public surface for future consumers, and `ops`
  consumes it internally via `crate::engine`. No action required; just noting it
  is deliberately public, not an oversight.
- **`cli/Cargo.toml` serde feature flag.** The only direct `serde` use in
  resident files is the `T: serde::Serialize` trait bound (mcp.rs:329), not a
  derive. Preserving the existing `features = ["derive"]` is harmless and
  recommended for future-proofing; dropping it would also compile. Ensure the
  feature flag is not accidentally regressed when editing `cli/Cargo.toml`.
- **`ignore` is used in `mcp.rs` as well as `init.rs`.** The plan retains
  `ignore` in `cli/` — correct. Just confirming the retention is justified by
  both resident files, not only `init.rs`.