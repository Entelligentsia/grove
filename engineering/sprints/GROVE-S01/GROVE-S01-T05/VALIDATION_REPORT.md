# Validation Report — GROVE-S01-T05 (iteration 1 of 3)

**Verdict:** Approved

## Validation Summary

All 7 acceptance criteria validated against the sprint requirements and task plan. The CI workflow, release workflow, and bump-version.sh are now workspace-aware while maintaining zero end-user impact on asset names and paths.

## Acceptance Criteria Validation

### AC#1: release.yml builds with -p grove

**Status:** PASS

Verified `.github/workflows/release.yml` line 48:
```yaml
run: cargo build --release --locked -p grove --target ${{ matrix.target }}
```
- The `-p grove` flag pins the release artifact to the grove package
- Asset path `target/<target>/release/grove[.exe]` unchanged
- Upload glob `grove-<target>.*` unchanged (line 77)
- All 5 targets in the matrix: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin, x86_64-pc-windows-msvc

### AC#2: ci.yml runs workspace build and test

**Status:** PASS

Verified `.github/workflows/ci.yml`:
- Line 37: `cargo build --release --locked --workspace`
- Line 40: `cargo test --release --locked --workspace`

Both workspace members (grove and grove-core) are built and tested.

### AC#3: bump-version.sh handles both crates

**Status:** PASS

Verified `scripts/bump-version.sh` is a 5-step script:
1. Step 1 (line 32-35): `cli/Cargo.toml` (grove)
2. Step 2 (line 37-39): `core/Cargo.toml` (grove-core)
3. Step 3 (line 41-54): `cargo update -p grove -p grove-core` with assertions for both lock entries
4. Step 4 (line 56-58): `dist/npm/package.json`
5. Step 5 (line 60-74): `CHANGELOG.md`

Idempotency test with `scripts/bump-version.sh 0.1.11` passed — all version sites at 0.1.11, no spurious changes.

### AC#4: setup-local-test.sh resolves target/release/grove

**Status:** PASS

Verified `scripts/setup-local-test.sh` line 28:
```bash
BIN="$REPO/target/release/grove"
```
Under a virtual workspace, `cargo build --release` at the workspace root produces the grove binary at `target/release/grove` — unchanged.

### AC#5: npm and Homebrew resolve grove binary unchanged

**Status:** PASS

- `dist/npm/install.js` line 68: `const asset = \`grove-\${target}.\${ext}\`;` — asset names unchanged
- `dist/homebrew/update-formula.sh` line 18: `curl -fsSL "$base/grove-$1.tar.gz.sha256"` — asset names unchanged

### AC#6: Dry validation passes

**Status:** PASS

- `cargo build --release --locked -p grove` succeeds, binary at `target/release/grove`
- Release.yml asset-name review confirmed (see AC#1)
- YAML files parse without errors

### AC#7: Workspace tests and clippy pass

**Status:** PASS

Test results:
```
running 32 tests (CLI/mcp unit) — ok
running 18 tests (CLI integration) — ok
running 87 tests (grove-core unit) — ok
Doc-tests grove_core — 1 passed
```
Total: 138 tests, all passing.

Clippy: Clean, no warnings.

## Sprint Requirements Traceability

From SPRINT_REQUIREMENTS.md Step 5 acceptance criteria:
- [x] `.github/workflows/release.yml` builds the workspace and produces the `grove` binary for all 5 target platforms
- [x] `scripts/bump-version.sh` bumps the version in both `core/Cargo.toml` and `cli/Cargo.toml` (and `Cargo.lock` + `dist/npm/package.json`) so they stay in sync
- [x] `dist/npm` and Homebrew release scripts still resolve and package the built `grove` binary correctly
- [x] A CI test job runs `cargo test --release --locked --workspace` green

## Conclusion

All acceptance criteria validated. The implementation correctly makes CI, release workflow, and bump-version.sh workspace-aware while maintaining zero end-user impact on asset names, paths, and distribution workflows.
