# PLAN — GROVE-S01-T06: crates.io publication readiness for grove-core + grove

🗻 *grove Architect*

**Task:** GROVE-S01-T06
**Sprint:** GROVE-S01
**Estimate:** M

---

## Objective

Make both workspace crates (`grove-core` and `grove`) fully publishable to crates.io by fixing the one blocking gap — a missing version pin on `grove`'s internal `grove-core` dependency — adding a version-pin maintenance step to `bump-version.sh`, and documenting the crates.io publish order in `RELEASING.md`.

## Approach

Research (already completed in planning) established the following:

1. **`grove-core` dry-run already passes.** `core/Cargo.toml` has all required crates.io metadata (`description`, `license`, `repository`, `homepage`, `readme`, `keywords`, `categories`). `cargo publish --dry-run -p grove-core` compiles and packages successfully.

2. **`grove` dry-run fails with one error.** `cli/Cargo.toml` has all required metadata fields but the `grove-core` dependency is declared as `grove-core = { path = "../core" }` — no `version`. crates.io rejects path-only dependencies on publish. The fix is to add an exact version pin matching the workspace version: `grove-core = { path = "../core", version = "=0.1.11" }`.

3. **`bump-version.sh` does not update the version pin.** The script bumps `[package].version` in both `Cargo.toml` files and refreshes `Cargo.lock`, but never touches the `grove-core` dependency version in `cli/Cargo.toml`. This would silently drift the pin after every release. The script must gain a step to keep them in sync.

4. **Both crate names are unclaimed on crates.io.** API queries for `grove` and `grove-core` return no existing crate. Publication can proceed under these names with no fallback required.

5. **`RELEASING.md` has no crates.io steps.** The runbook documents GitHub Release, npm, and Homebrew but omits `cargo publish`. The crate ordering constraint (core must precede cli because the published cli resolves the published lib by version) must be documented before the next release.

Implementation is three focused edits and one verification pass:

- Fix the version pin in `cli/Cargo.toml`.
- Extend `scripts/bump-version.sh` to update the pin atomically with the rest.
- Add a `cargo publish` section to `RELEASING.md`.
- Run `cargo publish --dry-run -p grove-core` and `cargo publish --dry-run -p grove` to confirm both succeed; run `cargo test --release --locked --workspace` to confirm nothing regressed.

No engine changes, no API changes, no user-facing behaviour change.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `cli/Cargo.toml` | Change `grove-core = { path = "../core" }` to `grove-core = { path = "../core", version = "=0.1.11" }` | crates.io requires a version on all dependencies; exact pin ensures published grove binds the matching grove-core |
| `scripts/bump-version.sh` | Add a step (between current steps 2 and 3) that updates the `version = "=…"` in the `grove-core` dependency line of `cli/Cargo.toml` to match the new version | Without this, every `bump-version.sh` run leaves the pin stale |
| `RELEASING.md` | Add a numbered publish step (`cargo publish -p grove-core`, then `cargo publish -p grove`) and an ordering constraint note after the tag/release step | Documents the crate publish lifecycle for future releases; notes that core must publish before cli |

## Plugin Impact Assessment

- **Version bump required?** No — this task prepares for publication but does not cut a release; the current `0.1.11` version is the publication candidate.
- **Migration entry required?** No.
- **Security scan required?** No — no Forge plugin files changed.
- **Schema change?** No.

## Testing Strategy

- **Dry-run smoke tests (required gate):**
  ```
  cargo publish --dry-run -p grove-core
  cargo publish --dry-run -p grove
  ```
  Both must exit 0 (the final line is `warning: aborting upload due to dry run`).
- **Workspace build and test (regression gate):**
  ```
  cargo build --release --locked --workspace
  cargo test --release --locked --workspace
  ```
  Must pass exactly as in T01–T05.
- **Lint gate:**
  ```
  cargo clippy --workspace -- -D warnings
  ```
- **bump-version.sh sanity (manual spot-check, not automated):** run `scripts/bump-version.sh 0.1.12` in a throw-away branch; verify that `cli/Cargo.toml`'s grove-core pin becomes `=0.1.12`; revert.

## Acceptance Criteria

- [ ] `cli/Cargo.toml` contains `grove-core = { path = "../core", version = "=0.1.11" }`.
- [ ] `cargo publish --dry-run -p grove-core` exits 0 with `warning: aborting upload due to dry run`.
- [ ] `cargo publish --dry-run -p grove` exits 0 with `warning: aborting upload due to dry run`.
- [ ] `scripts/bump-version.sh` includes a step that updates the `version = "=…"` pin in `cli/Cargo.toml`'s grove-core dependency entry, with a matching `say` and `grep` confirmation.
- [ ] `RELEASING.md` has a `cargo publish` step that: (a) publishes `grove-core` first, (b) publishes `grove` second, (c) includes the ordering constraint note explaining why core must precede cli.
- [ ] `cargo test --release --locked --workspace` passes (all 18 tests green).
- [ ] `cargo clippy --workspace -- -D warnings` exits 0.
- [ ] Crate name availability confirmed in plan documentation: both `grove` and `grove-core` are unclaimed on crates.io.

## Operational Impact

- **Distribution:** no impact to existing binary, npm, or Homebrew users — this task only enables a new `cargo install grove` / `Cargo.toml` dependency path.
- **Backwards compatibility:** purely additive; existing distribution channels are unchanged.
- **Publish ordering:** when the actual `cargo publish` is executed (post-approval by maintainer), `grove-core` must be published first so that crates.io can resolve it when `grove` is published. This is now documented in `RELEASING.md`.
