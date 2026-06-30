# Architect Approval: GROVE-S01-T01 — Establish Cargo workspace; relocate crate into cli/

**Verdict:** Approved

## Rationale

This task establishes the foundational Cargo workspace structure that enables the entire sprint's objective: extracting grove-core as a reusable library. The implementation is minimal, correct, and positions the codebase for T02-T06.

### Architectural Assessment

1. **Virtual workspace pattern** — Using a root `Cargo.toml` with only `[workspace]` (no `[package]`) is the canonical approach for multi-crate repositories. It allows `grove-core` (T02) to be added as a sibling member with shared dependencies and a single lockfile.

2. **Profile placement** — Moving `[profile.release]` to the workspace root was the correct decision. Cargo ignores profile sections in member crates; this ensures lto=thin and strip=true continue to apply.

3. **Path handling** — The DEV_REGISTRY fix (`/../registry`) is sound. CARGO_MANIFEST_DIR resolves to the member's directory at compile time; the relative path correctly reaches the repo-root dev stub.

4. **Build stability** — 134 tests pass, clippy is clean, and the binary output path is unchanged. No regressions.

### Cross-Cutting Concerns

- **CI/Release** — The existing `cargo build/test --release --locked` commands continue to work at the workspace root. Explicit `--workspace` can be added in T05 for safety but is not required for correctness.

- **Cargo.lock** — Correctly remains at the workspace root; no duplication in cli/. Dependency resolution is unified.

- **Publishing** — The `readme = "../README.md"` adjustment ensures `cargo publish` from cli/ can locate the readme. No immediate publication impact; T06 handles crates.io readiness.

## Deployment Notes

No deployment changes. The binary path (`target/release/grove`) is unchanged. The Homebrew formula, npm wrapper, and release workflows all continue to work as-is.

## Follow-Up Items

- T02 will extract grove-core; ensure ops.rs/engine.rs boundaries are clean when splitting.
- T05 should add explicit `--workspace` flags to CI for multi-member correctness.
- Consider adding workspace-level metadata (`workspace.package`) in T06 for DRY versioning.