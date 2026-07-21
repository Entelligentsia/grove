# GROVE-S04-T08: Packaging — `release.yml`/npm/brew ship both binaries; release dry-run

**Sprint:** GROVE-S04
**Estimate:** M
**Pipeline:** default

---

## Objective

Make the split release-ready (intake decision D2): the existing release
pipeline builds, packages, and distributes **both** binaries — `grove` and
`grove-explore` — across all 5 platform targets, npm, and the Homebrew
formula, proven by a dry-run. No version is actually published (D3).

## Acceptance Criteria

1. `release.yml` builds and uploads both binaries for all 5 targets
   (linux x86_64/aarch64, macos x86_64/aarch64, windows x86_64), with per-file
   sha256 checksums; Unix artifacts `.tar.gz`, Windows `.zip` (both binaries
   in the platform archive, or parallel archives — decided in plan, applied
   consistently across npm/brew/install script).
2. `dist/npm` installs both binaries on `npm install` (platform download
   wrapper extended); `package.json` `bin` maps both `grove` and
   `grove-explore`.
3. `dist/homebrew/update-formula.sh <tag>` produces a formula that installs
   both binaries; the curl install script (if it enumerates artifact names)
   handles the new layout.
4. A release dry-run proves the pipeline end-to-end before sprint close: the
   workflow runs against a non-publishing ref (workflow_dispatch dry-run input,
   draft release, or CI-equivalent job) producing all artifacts; evidence
   linked in PROGRESS.md. No tag is pushed, nothing is published (D3).
5. `RELEASING.md` is updated for the two-binary release train (build, npm,
   brew steps).
6. CI (`ci.yml`) builds and tests the whole workspace including the new bin
   target.
7. Workspace green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean build, files end with a newline.

## Context

Implements item 8 of `SPRINT_REQUIREMENTS.md` (D2). Depends on **T02/T03**
(final binary set) and **T05** (final verb surface — packaging an intermediate
binary would ship verbs that move). Interim single-repo form: both binaries
ship together from this repo's release train; total install size for
both-server users is unchanged, `grove` itself shrinks (ratatui/crossterm
left). Stack checklist rows: binary packaging, checksums, npm wrapper version
refs, formula tag/sha256 match.

## Artifacts Involved

- `.github/workflows/release.yml`, `.github/workflows/ci.yml`
- `dist/npm/` (wrapper + package.json), `dist/homebrew/update-formula.sh`,
  install script
- `RELEASING.md`

## Operational Impact

- **Version bump:** none in this sprint (dry-run only, D3); the next real
  release ships both binaries.
- **Regeneration:** npm/brew users get both binaries on their next upgrade.
- **Security scan:** not required (no new network paths; checksums preserved).
