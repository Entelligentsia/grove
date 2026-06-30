# GROVE-S01-T05: Make CI & release tooling workspace-aware

**Sprint:** GROVE-S01
**Estimate:** M
**Pipeline:** default

---

## Objective

Update CI, the release workflow, and the version-bump script so the new
`core/` + `cli/` workspace builds, tests, releases, and version-bumps correctly —
while the published artifact stays a single binary named `grove` with an unchanged
download path. Keeps the release pipeline green for the next tag.

## Acceptance Criteria

1. `.github/workflows/release.yml` builds the workspace and emits the `grove` binary
   for all 5 targets. Prefer `cargo build --release --locked -p grove --target <t>`;
   the asset path `target/<target>/release/grove` (and `grove.exe`) is unchanged, so
   the existing `tar`/`7z` steps keep working.
2. `.github/workflows/ci.yml` runs `cargo build --release --locked --workspace` and
   `cargo test --release --locked --workspace` (was non-workspace single-crate).
3. `scripts/bump-version.sh` bumps the version in **both** `core/Cargo.toml` and
   `cli/Cargo.toml` (root no longer has `[package]`), plus the `grove` **and**
   `grove-core` entries in `Cargo.lock` (via `cargo update -p ...`) and
   `dist/npm/package.json` — all to the same version, in one shot, idempotently.
4. `scripts/setup-local-test.sh` still resolves `target/release/grove` (verify the
   `cargo build --release` it runs produces the binary at the same path under the
   workspace).
5. `dist/npm` install wrapper and `dist/homebrew/update-formula.sh` are verified to
   still resolve/package the `grove` binary (they pull GitHub Release assets by name,
   so no path change expected — confirm and note).
6. A dry validation passes: `cargo build --release --locked -p grove` locally, and a
   documented review of `release.yml` confirming the asset names are unchanged.

## Context

Depends on **T02** (the `core/`+`cli/` layout and dual `Cargo.toml`s exist after
T02; the `init` split in T03 doesn't affect build layout, so T05 can run in
parallel with T03/T04). Grounding from the current tree: `release.yml` tars
`target/<target>/release/grove`; `ci.yml` runs bare `cargo build/test --release
--locked`; `bump-version.sh` edits root `Cargo.toml [package] version` + `Cargo.lock`
grove entry + npm — that root-`[package]` assumption is now invalid and must change.

## Artifacts Involved

- Edited: `.github/workflows/release.yml`, `.github/workflows/ci.yml`,
  `scripts/bump-version.sh`.
- Verify (likely unchanged): `scripts/setup-local-test.sh`, `dist/npm/*`,
  `dist/homebrew/update-formula.sh`, `dist/homebrew/grove.rb`.

## Operational Impact

- **Version bump:** this task *changes how* bumps happen; no actual release here.
- **Regeneration:** none for end users.
- **Backward compat:** release asset names + npm/Homebrew install paths unchanged
  (a gating criterion); verify before any future tag.
