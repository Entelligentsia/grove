# Plan Review: GROVE-S01-T01 — Establish Cargo workspace; relocate crate into cli/

**Review iteration:** 1 (standalone review)  
**Reviewed:** 2026-06-30

## Verdict: Approved

The plan is technically sound, correctly scoped, and executable as written.

## Assessment

### Feasibility
The plan describes a pure structural refactor with no code logic changes. Each step maintains a buildable state before proceeding to the next. The risk surface is narrow and well-identified.

### Completeness
- All 8 source files (`main.rs`, `ops.rs`, `mcp.rs`, `engine.rs`, `registry.rs`, `fetch.rs`, `ingest.rs`, `init.rs`) are accounted for.
- The `DEV_REGISTRY` constant fix in `tests/cli.rs` is correctly identified and the solution (`concat!(env!("CARGO_MANIFEST_DIR"), "/../registry")`) is valid.
- `scripts/bump-version.sh` update is correctly scoped to target `cli/Cargo.toml`.
- Acceptance criteria are testable and align with sprint requirements.

### Technical Accuracy
- Virtual workspace with `members = ["cli"]` and `resolver = "2"` is the correct Cargo workspace pattern.
- Keeping `Cargo.lock` at workspace root is correct (workspace-level lockfile).
- The `readme = "../README.md"` pattern for `cli/Cargo.toml` is correct for `cargo publish`.
- `CARGO_BIN_EXE_grove` resolution works correctly for workspace members.

### Architecture Alignment
The plan correctly implements Step 1 of the sprint requirements:
- Creates virtual workspace at root
- Relocates existing crate to `cli/`
- Preserves binary name and output path
- Does NOT touch `core/` extraction (that is T02)

### Non-Goals
Correctly excludes:
- T02's `grove-core` extraction
- CI/release workflow changes (T05)
- Any source logic changes

## Advisory Notes

1. **Execution order matters:** The plan states to create both manifests before moving `src/`. This is important — moving `src/` first would leave Cargo unable to find the crate.

2. **Verify `cargo clippy` post-move:** While the plan mentions it, ensure clippy is run from workspace root after all changes, not just from `cli/`.

3. **Test registry stub path:** After fixing `DEV_REGISTRY`, verify that the 3-language dev stub tests (rust, python, javascript) all pass with their expected counts.
