# ARCHITECT_APPROVAL — GROVE-S04-T08

**Verdict:** Approved

## Architectural Assessment

The dual-binary packaging implementation for grove (structural tools) and grove-explore (mcp-llm mode) is architecturally sound and ready for deployment.

### Implementation Alignment

The task correctly implements the two-binary architecture established in GROVE-S04-T01 through T07:

1. **Binary separation**: `grove` (CLI + MCP server for structural tools) and `grove-explore` (MCP server for explore mode) are distinct release artifacts with their own version reporting.

2. **Release train**: The GitHub Actions workflow (`release.yml`) builds both binaries for all five platform targets (linux-x86_64, linux-aarch64, darwin-x86_64, darwin-aarch64, windows-x86_64), with correct 4-crate publish order (grove-cst, grove-explore-core, grove-cst-cli, grove-explore-bin).

3. **Distribution channels**: npm, Homebrew, and curl install all ship both binaries. The install scripts correctly handle older archives that may lack grove-explore (graceful degradation).

### Cross-Cutting Concerns

- **Version consistency**: All four workspace crates, Cargo.lock, and npm package.json are synchronized at 0.4.1. The bump-version.sh script handles all twelve version-bearing locations correctly.

- **CI gate**: Full test suite (97 tests) and clippy lint pass. The release workflow dry-run skip prevents accidental releases from non-tag pushes.

- **Documentation**: RELEASING.md now includes the "Testing the bump script" section that codifies the safe run-verify-restore-checksum protocol, preventing recurrence of the version-corruption incident.

### Operational Impact

- **Deployment posture**: No database migrations, no infrastructure changes. The release is a pure artifact addition.
- **Rollback path**: Standard release rollback via GitHub Release deletion + npm unpublish + Homebrew formula revert.
- **Breaking changes**: None. Existing `grove` users are unaffected; `grove-explore` is additive.

## Deployment Notes

- The next `vX.Y.Z` tag push will trigger the full release train.
- npm publish requires manual `npm publish` from `dist/npm/`.
- Homebrew tap regeneration requires `dist/homebrew/update-formula.sh vX.Y.Z`.

## Follow-Up Items (Future Sprints)

1. **Automated npm publish**: Consider adding npm publish to the release workflow (requires npm token secret).
2. **Homebrew tap automation**: The formula regeneration is currently manual post-release.
3. **Binary size monitoring**: The two-binary approach duplicates wasmtime; track archive sizes over releases.

---

Signed off: 2026-07-21T04:02:00.000Z
