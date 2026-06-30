# Plan: GROVE-S01-T01 — Establish Cargo workspace; relocate crate into cli/

## Objective

Convert the single-crate repository to a Cargo virtual workspace whose root
`Cargo.toml` declares `members = ["cli"]`, and relocate the current source tree
(`src/`, `tests/`, all `[dependencies]`, and the release profile) into the `cli/`
sub-crate. The binary name and output path remain unchanged; `cargo build/test
--release --locked` (with or without `--workspace`) must be green on completion.

## Approach

This is a pure structural refactor — no code logic changes. The risk surface is
narrow: path-sensitive constants in tests, a version-bump script that hard-codes
`Cargo.toml`, and the CI/release workflows. All must be updated in lockstep with
the file moves.

Execute in this order to stay in a buildable state at each step:

1. Create the workspace root manifest.
2. Create the `cli/` sub-crate manifest.
3. Move source files.
4. Fix the broken test path constant.
5. Update the version-bump script.
6. Verify locally with `cargo build --release --locked` and `cargo test --release
   --locked`.

## Files to Modify

| File | Action | Detail |
|------|--------|--------|
| `Cargo.toml` | **Replace** | Strip all `[package]`, `[[bin]]`, `[dependencies]`, `[profile.release]` content. Replace with a `[workspace]` table listing `members = ["cli"]` and `resolver = "2"`. Retain the existing `Cargo.lock` (workspace lock is at root — no move needed). |
| `cli/Cargo.toml` | **Create** | New file. Carry over the full `[package]` block (name, version, edition, description, license, repository, homepage, readme, keywords, categories), the `[[bin]]` block (`name = "grove"`, `path = "src/main.rs"`), all `[dependencies]`, and `[profile.release]`. The `readme` field should point to `../README.md` so `cargo publish` can find it from inside `cli/`. |
| `src/` → `cli/src/` | **Move directory** | All eight source files (`main.rs`, `ops.rs`, `mcp.rs`, `engine.rs`, `registry.rs`, `fetch.rs`, `ingest.rs`, `init.rs`) move as-is; no source edits needed inside these files. |
| `tests/` → `cli/tests/` | **Move directory** | Move `tests/cli.rs` into `cli/tests/cli.rs`. |
| `cli/tests/cli.rs` | **Edit one constant** | `DEV_REGISTRY` currently uses `concat!(env!("CARGO_MANIFEST_DIR"), "/registry")`. After the move `CARGO_MANIFEST_DIR` resolves to `<repo>/cli`, but `registry/` stays at the repo root. Change to `concat!(env!("CARGO_MANIFEST_DIR"), "/../registry")`. The `/../` is resolved by the OS at runtime; this is the minimal, compile-time-safe fix. |
| `scripts/bump-version.sh` | **Edit two references** | The script `perl`-edits `Cargo.toml` for the package version and verifies `Cargo.lock`. After the refactor the package version lives in `cli/Cargo.toml`, not the workspace root (the root has no `[package]`). Update the `perl` invocation and the subsequent `grep` to target `cli/Cargo.toml`. The `Cargo.lock` verification stays at the root — no change there. |

## Data Model Changes

None. No store schema, config, or entity model is affected by this task.

## Testing Strategy

Run the full test suite from the workspace root after each step:

```sh
# After creating root Cargo.toml and cli/Cargo.toml (before moving src/):
# — workspace is malformed; do not run tests mid-step.

# After moving src/ and tests/:
cargo build --release --locked
cargo test --release --locked
```

The integration suite in `cli/tests/cli.rs` shells out to the compiled binary via
`CARGO_BIN_EXE_grove`, which Cargo resolves correctly for workspace members. The
`DEV_REGISTRY` path fix is the single non-trivial test change; confirm that the
outline/symbols/callers/definition tests all pass against the dev stub.

`cargo clippy -- -D warnings` must also be clean after the move (no source
changes means no new warnings, but confirm).

## Acceptance Criteria

- `Cargo.toml` at the repo root contains only a `[workspace]` section (no
  `[package]`).
- `cli/Cargo.toml` exists with `name = "grove"` and reproduces all current
  package metadata and dependencies.
- `src/` no longer exists at the repo root; `cli/src/` does.
- `tests/` no longer exists at the repo root; `cli/tests/` does.
- `cargo build --release --locked` from the repo root succeeds and produces
  `target/release/grove`.
- `cargo test --release --locked` from the repo root passes all tests (same
  count as before).
- `cargo clippy -- -D warnings` exits 0.
- `scripts/bump-version.sh` edits `cli/Cargo.toml` for the version field (not
  the workspace root manifest).
- `Cargo.lock` remains at the repo root (workspace-level lock) and is not
  duplicated inside `cli/`.

## Operational Impact

None at runtime. The produced binary (`target/release/grove`) is identical. CI
and release workflows invoke `cargo build --release --locked` at the repo root
and rely on the binary at the standard target path — both remain correct for a
virtual workspace. No versioned release artifact changes.

## Non-Goals for This Task

- Extracting `grove-core` — that is T02.
- Changing any source logic, public API, or MCP protocol.
- Adding or removing dependencies.
- Updating CI workflow files (the existing `cargo build/test --release --locked`
  commands work unchanged at workspace root; explicit `--workspace` is deferred
  to T05 which is the CI/release pass).
