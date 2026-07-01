# PLAN — GROVE-S01-T05: Make CI & release tooling workspace-aware

🌱 *grove Engineer*

**Task:** GROVE-S01-T05
**Sprint:** GROVE-S01
**Estimate:** M

---

## Objective

Make the CI workflow, the release workflow, and the version-bump script
correct for the new `core/` (grove-core lib) + `cli/` (grove bin) Cargo
workspace, while guaranteeing the published artifact remains a single binary
named `grove` whose download path and release asset names are unchanged. The
outcome is a release pipeline that stays green for the next tag with zero
end-user impact.

## Approach

The repo is already a virtual workspace (root `Cargo.toml` is `[workspace]`
with members `core` + `cli`; `core/Cargo.toml` = `grove-core`, `cli/Cargo.toml`
= `grove` with `[[bin]] name = "grove"`; `Cargo.lock` carries both `grove` and
`grove-core` at 0.1.11). The job is to align the three pieces of tooling that
still assume the old single-crate layout, and to verify (no edits expected) the
distribution wrappers that pull release assets by name.

Key insight that keeps end-user impact at zero: in a workspace, only `cli`
declares a `grove` binary, so `cargo build --release` at the workspace root —
and `cargo build --release -p grove --target <t>` — both still emit the binary
at `target/<target>/release/grove` (and `grove.exe`). Therefore the existing
`tar`/`7z` packaging steps, the npm asset name `grove-<target>.<ext>`, and the
Homebrew `grove-<target>.tar.gz.sha256` lookups need no changes — only
confirmation.

Concretely:
1. **release.yml** — make the build step target the `grove` package explicitly
   so a future second binary in the workspace can never change the asset, by
   switching `cargo build --release --locked --target <t>` to
   `cargo build --release --locked -p grove --target <t>`. Packaging paths stay
   byte-for-byte identical.
2. **ci.yml** — broaden the build and test from implicit single-crate to the
   whole workspace: `cargo build --release --locked --workspace` and
   `cargo test --release --locked --workspace`, so `grove-core`'s tests run too.
3. **bump-version.sh** — it already bumps `cli/Cargo.toml`; extend it to also
   bump `core/Cargo.toml` and to update **both** lock entries
   (`cargo update -p grove -p grove-core`), keeping all four version sites
   (`core`, `cli`, lockfile, npm) in lockstep and idempotent. Tighten the
   lockfile assertion to check the `grove-core` entry as well.
4. **Verify-only** — confirm `scripts/setup-local-test.sh`,
   `dist/npm/install.js`, `dist/npm/package.json`, and
   `dist/homebrew/update-formula.sh` resolve/package the `grove` binary
   unchanged, and record that confirmation in PROGRESS.md.

No code/pseudocode is committed here; the script edits are mechanical and
described, not written, in this plan.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `.github/workflows/release.yml` | Build step → `cargo build --release --locked -p grove --target ${{ matrix.target }}` | Pin the release artifact to the `grove` package; asset path `target/<t>/release/grove[.exe]` unchanged so tar/7z steps keep working (AC#1). |
| `.github/workflows/ci.yml` | Build step → `cargo build --release --locked --workspace`; Test step → `cargo test --release --locked --workspace` | CI must build & test both workspace members, not just the implicit crate (AC#2). |
| `scripts/bump-version.sh` | Add a `core/Cargo.toml` version bump (mirroring the existing `cli/Cargo.toml` perl edit); change `cargo update -p grove` → `cargo update -p grove -p grove-core`; assert both `grove` and `grove-core` lock entries equal the target version; update the `say "N/M"` step labels | Root no longer has `[package]`; both crate versions + both lock entries + npm must move to the same version in one idempotent shot (AC#3). |

### Verify-only (expected unchanged — confirm and note, no edits planned)

| File | Verification |
|---|---|
| `scripts/setup-local-test.sh` | `cargo build --release` at workspace root still produces `target/release/grove`; the `BIN` path resolves (AC#4). |
| `dist/npm/install.js` | Downloads `grove-${target}.${ext}` and installs `grove`/`grove.exe` — asset names tied to release.yml, unchanged (AC#5). |
| `dist/npm/package.json` | `bin.grove → bin/grove.js`; version touched only by bump script — no layout dependency. |
| `dist/homebrew/update-formula.sh` + `grove.rb` | Pull `grove-<target>.tar.gz.sha256` by name from the GitHub Release — unchanged (AC#5). |

## Data Model Changes

None. No `.forge/store/` schema, `.forge/config.json`, or entity-model change.
This task touches CI/release tooling and a build-time version-bump script only;
there is no application data model involved.

## Plugin / Version Impact Assessment

- **Material change?** Yes — workflow and release-tooling/script changes that
  alter build & release behaviour are material per the stack checklist.
- **Version bump required (in this task)?** No. This task *changes how* bumps
  happen but performs no release; the next tag will exercise the updated
  `bump-version.sh`. No `## [X.Y.Z]` entry is added here.
- **Schema change?** No.
- **Migration entry required?** No.

## Testing Strategy

- **Workspace build (dry validation, AC#6):** `cargo build --release --locked -p grove`
  succeeds locally and the binary appears at `target/release/grove`.
- **Full workspace gate:** `cargo test --release --locked --workspace` (the
  project test command, broadened to `--workspace`) passes; `cargo clippy -- -D warnings`
  clean.
- **bump-version.sh dry run:** run `scripts/bump-version.sh 0.1.11` (same
  version → idempotent) and confirm it edits `core/Cargo.toml`, `cli/Cargo.toml`,
  both `Cargo.lock` entries, and `dist/npm/package.json` to 0.1.11 with no error,
  then `git diff` shows no spurious changes. Optionally bump to a throwaway
  version, inspect the diff across all four sites, and revert.
- **release.yml review (AC#6):** documented read-through confirming the asset
  path `target/<target>/release/grove[.exe]` and `grove-<target>.*` upload glob
  are unchanged by the `-p grove` switch.
- **YAML/script sanity:** workflows parse (yamllint or GitHub schema), and
  `bash -n scripts/bump-version.sh` passes.

## Acceptance Criteria

- [ ] `release.yml` builds with `-p grove` for all 5 targets; asset path and
  `grove-<target>.*` upload glob unchanged (AC#1).
- [ ] `ci.yml` runs `cargo build --release --locked --workspace` and
  `cargo test --release --locked --workspace` (AC#2).
- [ ] `bump-version.sh` moves `core/Cargo.toml`, `cli/Cargo.toml`, both
  `Cargo.lock` entries (`grove` + `grove-core`), and `dist/npm/package.json` to
  the same version in one idempotent run, with the lockfile assertion covering
  both entries (AC#3).
- [ ] `scripts/setup-local-test.sh` still resolves `target/release/grove` under
  the workspace (AC#4).
- [ ] npm install wrapper and `dist/homebrew/update-formula.sh` confirmed to
  resolve/package the `grove` binary unchanged, noted in PROGRESS (AC#5).
- [ ] Dry validation passes: `cargo build --release --locked -p grove` locally +
  documented release.yml asset-name review (AC#6).
- [ ] `cargo test --release --locked --workspace` and `cargo clippy -- -D warnings`
  are green.

## Operational Impact

- **Version bump:** this task changes *how* bumps happen (now multi-crate
  aware); no actual release is cut here.
- **Regeneration:** none for end users.
- **Backward compat (gating):** release asset names (`grove-<target>.tar.gz`,
  `.zip`, `.sha256`) and npm/Homebrew install paths are unchanged — the binary
  stays a single `grove`. This is a hard gate; any divergence fails the task.
- **Distribution:** no `/forge:update`-style action required of users; effect is
  internal to the maintainer release pipeline and is realised on the next tag.
