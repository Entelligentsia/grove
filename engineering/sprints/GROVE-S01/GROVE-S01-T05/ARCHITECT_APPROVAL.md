# ARCHITECT_APPROVAL.md — GROVE-S01-T05

## Overview

Task GROVE-S01-T05 makes the CI, release, and version-bump tooling workspace-aware
for the new `core/` (grove-core) + `cli/` (grove) Cargo workspace structure.

## Architectural Assessment

### Alignment with Project Architecture

The implementation correctly adapts the tooling layer to the new workspace structure
established in T01–T04 while preserving the critical invariant: **the published
artifact remains a single binary named `grove` with unchanged asset names**.

Key architectural observations:

1. **Single binary output preserved**: The `-p grove` flag in release.yml explicitly
   pins the release artifact, preventing future workspace member additions from
   accidentally changing the published asset.

2. **Full workspace coverage in CI**: Using `--workspace` ensures both `grove-core`
   and `grove` are built and tested, catching integration issues between the library
   and CLI layers.

3. **Version lockstep enforcement**: The bump-version.sh script's 5-step approach
   with explicit assertions for both lock entries prevents version drift between
   workspace members.

### Cross-Cutting Concerns

- **No downstream impact**: Asset names (`grove-<target>.tar.gz`, `.zip`, `.sha256`)
  are unchanged. npm/Homebrew install paths continue to work.
- **Distribution unchanged**: The `setup-local-test.sh`, `dist/npm/install.js`,
  and `dist/homebrew/update-formula.sh` all resolve the binary correctly under
  the virtual workspace.

### Operational Impact

- **Release pipeline**: The changes are internal to the maintainer release pipeline;
  end users see no difference.
- **Version bumps**: Now require awareness of both crate versions, but the script
  handles this atomically.
- **No migration required**: No user action needed.

## Deployment Notes

- The next `vX.Y.Z` tag push will exercise the updated release.yml workflow.
- All 5 platform targets continue to build and upload identically.
- The bump-version.sh script is idempotent — running it with the current version
  produces no spurious diffs.

## Follow-up Items for Future Sprints

- T06 (crates.io publication readiness) depends on this task and will establish
  the publication workflow for `grove-core` as a library crate.

---

**Verdict:** Approved
