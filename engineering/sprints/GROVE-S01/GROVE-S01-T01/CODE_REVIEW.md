# Code Review: GROVE-S01-T01 — Establish Cargo workspace; relocate crate into cli/

**Reviewer:** Supervisor (iteration 1 of 3)
**Verdict:** Approved

## Summary

Clean structural refactor converting the single-crate repository to a Cargo virtual workspace. All acceptance criteria verified independently by reading actual files and running build/test commands.

## Spec Compliance (verified independently)

| Acceptance Criterion | Status | Evidence |
|---------------------|--------|----------|
| Root Cargo.toml contains only [workspace] section | PASS | `git diff -- Cargo.toml` shows [package]/[[bin]]/[dependencies] replaced with `[workspace]` + `members = ["cli"]` + `resolver = "2"` |
| cli/Cargo.toml exists with package metadata | PASS | File read confirms name="grove", version="0.1.11", all deps present, readme="../README.md" |
| src/ no longer exists at root | PASS | `ls src/` returns "No such file or directory" |
| cli/src/ contains all 8 source files | PASS | `ls -la cli/src/` shows engine.rs, fetch.rs, ingest.rs, init.rs, main.rs, mcp.rs, ops.rs, registry.rs |
| tests/ no longer exists at root | PASS | Verified via git status showing `D tests/cli.rs` |
| cli/tests/ contains cli.rs | PASS | File read confirms DEV_REGISTRY uses `"/../registry"` path |
| cargo build --release --locked succeeds | PASS | Ran command, exited 0, binary at target/release/grove (12.5MB) |
| cargo test --release --locked passes | PASS | 134 tests (117 unit + 17 CLI), all passed |
| cargo clippy -- -D warnings exits 0 | PASS | Ran command, clean output |
| bump-version.sh targets cli/Cargo.toml | PASS | `git diff -- scripts/bump-version.sh` shows perl/grep now use `cli/Cargo.toml` |
| Cargo.lock at root, not in cli/ | PASS | Verified both paths |

## Code Quality

### Correct decisions

1. **[profile.release] moved to workspace root** — Cargo ignores profile sections in non-root packages. Placing it in the workspace manifest ensures lto=thin and strip=true apply to the release build. This is the correct fix.

2. **DEV_REGISTRY path** — `concat!(env!("CARGO_MANIFEST_DIR"), "/../registry")` correctly resolves to the repo-root registry/ when CARGO_MANIFEST_DIR is `<repo>/cli`. The `/../` is normalized by the OS at runtime.

3. **readme = "../README.md"** — Necessary for `cargo publish` to locate the README from within the cli/ subdirectory.

4. **Clippy fixes** — Two pre-existing lints were fixed opportunistically:
   - `0..=2` range pattern (mcp.rs:344) — cleaner than `0 | 1 | 2`
   - Removed unused `.enumerate()` (ops.rs:453) — dead code removed

### Security

No security implications — this is a pure structural refactor with no logic changes.

### Architecture

Follows the established virtual workspace pattern. The setup positions the repo for T02 (grove-core extraction) by establishing the workspace foundation.

## Advisory Notes

None. The implementation is minimal and correct.
