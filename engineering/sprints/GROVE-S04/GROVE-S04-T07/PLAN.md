# PLAN — GROVE-S04-T07: `doctor` re-keying — explore checks keyed on `grove-explore` registration, not mode

🌱 *grove Engineer*

**Task:** GROVE-S04-T07  
**Sprint:** GROVE-S04  
**Estimate:** S

---

## Objective

Move `grove doctor`'s explore check group behind the new product boundary
established by ADR 0004 Stage 2. The explore checks must run only when the
project's selected harness configuration actually registers a `grove-explore`
MCP server, not when the declared mode happens to be `mcp-llm`. Structural
checks and the overall `--json` output shape remain unchanged, while the
harness-consistency drift matrix is extended for the two-registration layout
and a stale single-registration layout is flagged with a migration hint.

## Background and Context

- **GROVE-S03-T07** introduced `grove doctor` and the `core::doctor` module.
- **GROVE-S04-T03** removed `--explore` / `--standard` from `grove serve`,
  leaving `grove serve` as a structural-only MCP server.
- **GROVE-S04-T04** redefined `mcp-llm` to mean "register both servers": a
  `grove` entry for the structural surface and a separate `grove-explore`
  entry for the explore surface. Doctor must now verify that boundary by
  looking at registrations, not by assuming `mode == McpLlm` implies an
  explore surface.
- `doctor` remains read-only: it reports and hints, but never mutates files.

## Approach

### 1. Detect the `grove-explore` registration from every selected harness file

Add a harness-agnostic helper that scans each harness in the configured
harness set and reports whether any of them registers the `grove-explore`
server. The scan must cover all harness formats used by `reconcile_harness`:
Claude Code `.mcp.json`, Cursor `.cursor/mcp.json`, Codex global TOML, Gemini
`.gemini/settings.json`, Windsurf `.windsurf/mcp.json`, and VS Code
`.vscode/mcp.json`. The detection re-uses existing harness constants and
format metadata; no new dependency on `grove-explore-core` is introduced.

### 2. Re-key the explore check group

Change the explore check group's trigger from `mode == McpLlm` to the
presence of a `grove-explore` registration in any selected harness. When no
registration is present, the explore group is skipped entirely. The declared
mode is still loaded from `.grove/config.json` and is still reported in the
output, but it no longer gates the explore checks. The
`harness_serve_surface` check is updated to report based on registration
presence rather than mode inference.

### 3. Extend harness-consistency / drift detection for the split layout

Extend the per-harness registration checks to evaluate both the structural
`grove` entry and the explore `grove-explore` entry against the declared
mode. The drift matrix becomes:

| Declared mode | Expected structural entry | Expected explore entry | Drift outcome |
|---|---|---|---|
| `mcp-llm` | `grove` with args `["serve"]` | `grove-explore` present | Missing explore entry → fail with hint `grove init --as mcp-llm` |
| `mcp`, `skill`, `both`, `grammars` | `grove` (or none, per mode) | no `grove-explore` | Unexpected explore entry → fail with hint `grove init --as <mode>` |
| pre-split `mcp-llm` | single `grove` entry with old `--explore` arg | none | Fail with stale-layout hint to re-run `grove init --as mcp-llm` |

A stale-layout check looks for the literal `--explore` argument inside any
`grove` entry and emits the re-initialization hint instead of the generic
args-mismatch message.

### 4. Remove orphaned `--explore` / `--standard` force flags from `grove doctor`

The `grove doctor` CLI currently accepts `--explore` and `--standard` and
plumbs them into `core::config::ModeChoice` to force the effective mode.
Those flags are removed from the `Cmd::Doctor` branch in `cli/src/main.rs`
and the force plumbing is deleted.

For `core::config.rs`, take the conservative deletion path: remove only the
`ForceExplore` and `ForceStandard` variants from `ModeChoice`, leaving
`ModeChoice::None` as the sole variant. Keep the `active_mode(root,
ModeChoice) -> Mode` helper as a single-match function that reads the
declared mode. This path is self-contained: it does not require edits to
`core/src/lib.rs` or `cli/src/init.rs`, both of which still legitimately use
`ModeChoice::None` and `active_mode` in tests and re-exports.

Note: unlike `grove serve`, which keeps `--explore`/`--standard` as hidden
error-guards that redirect to `grove-explore serve`, `grove doctor` deletes
the flags outright because doctor has no successor command to redirect to.

### 5. Update tests and fixtures

Refresh `core/src/doctor.rs` unit tests and fixtures for:

- registration-gated group selection,
- two-registration drift rows for `mcp-llm`,
- unexpected `grove-explore` entries for non-`mcp-llm` modes,
- stale-layout detection for pre-split single-registration projects,
- a new multi-harness / non-Claude `mcp-llm` fixture that asserts the explore
group runs when `grove-explore` is registered only in Cursor (or another
non-Claude harness), and that drift detection still passes or fails per
harness.

In `cli/tests/cli.rs`, remove or update any assertions that depend on the
deleted `grove doctor --explore` / `--standard` flags (none remain), and keep
the existing `grove doctor` exit-code and `--json` shape tests green.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `core/src/doctor.rs` | Add a registration-detection helper; re-key the explore group trigger to the `grove-explore` registration; extend the drift matrix and stale-layout hint; update `harness_serve_surface` reporting; refresh unit tests and fixtures. | Central doctor logic and its tests live here. |
| `cli/src/main.rs` | Remove `--explore` / `--standard` flags and the `ModeChoice` plumbing from the `Cmd::Doctor` branch. | Consistency with T03's removal of the same flags from `grove serve`; no orphaned mode-forcing semantics remain. |
| `core/src/config.rs` | Drop only the `ForceExplore` / `ForceStandard` variants from `ModeChoice`; simplify `active_mode` to a single match arm; remove unit tests that cover the deleted force variants. | The only consumer of those two variants is the doctor force flags being deleted. Keeping `ModeChoice::None` and `active_mode` avoids a compile-breaking blast radius in `core/src/lib.rs` and `cli/src/init.rs`. |

The following files are intentionally **not** modified under the conservative
path:

- `core/src/lib.rs` — its `pub use config::{active_mode, GroveConfig, Mode, ModeChoice}` re-export remains valid because `active_mode` and `ModeChoice::None` are preserved.
- `cli/src/init.rs` — its test `reconcile_harness_then_save_config_active_mode` continues to call `active_mode(&dir, ModeChoice::None)`, which remains a valid public API.

## Data Model Changes

- `core::config::ModeChoice` is reduced to a single variant, `None`.
- `core::config::active_mode` keeps its existing signature but contains only
  one match arm (read declared mode from `GroveConfig`, fall back to `Mode::Mcp`).
- `core::doctor::diagnose` no longer accepts a force argument; it resolves the
  effective mode internally via `active_mode(root, ModeChoice::None)`.
- No changes to `.forge/store/`, `.forge/config.json`, `grove.lock`, the
  `Mode` enum, or the harness format constants.

## Plugin Impact Assessment

- **Version bump required?** Yes — `grove doctor` behaviour changes materially
  (explore checks gated by registration, new stale-layout hint).
- **Migration entry required?** No — doctor is read-only and only reports/hints.
- **Security scan required?** No — no change to the forge store, config schema,
  or network surface.
- **Schema change?** No — no `.forge/store/` or `.forge/config.json` schema
  change.

## Testing Strategy

### Unit tests in `core/src/doctor.rs`

- Registration-gated group selection: explore checks run iff a `grove-explore`
  server is registered in at least one selected harness.
- Multi-harness / non-Claude `mcp-llm` fixture: a project whose only
  `grove-explore` registration lives in Cursor `.cursor/mcp.json` (or across
  multiple harnesses) triggers the explore group and produces the expected
  per-harness drift results.
- Stale-layout detection: a single `grove` entry whose args contain the old
  `--explore` flag is flagged with the stale-layout hint.
- Two-registration drift rows:
  - `mcp-llm` missing `grove-explore` → fail.
  - non-`mcp-llm` with unexpected `grove-explore` → fail.
- Existing structural checks remain unchanged and continue to pass.

### Integration tests in `cli/tests/cli.rs`

- Confirm `grove doctor --help` no longer documents `--explore` or
  `--standard`.
- Confirm `grove doctor` still exits zero on a clean project and nonzero on a
  project with failures.
- Confirm JSON output shape and exit-code gate are preserved.

### Workspace-level checks

- `cargo test --release --locked`
- `cargo clippy --all-targets --workspace --locked -- -D warnings`
- Warning-clean release build.
- Every modified file ends with a newline.

### Manual smoke test

- Run `grove doctor` in a project initialized with `grove init --as mcp-llm`
  and verify the explore group is present.
- Run `grove doctor` in a project initialized with `grove init --as mcp` and
  verify the explore group is absent and no stale-layout false positive is
  reported.
- Run `grove doctor` in a pre-split `mcp-llm` project whose `.mcp.json` still
  has `grove` with args `["serve", "--explore"]` and verify the stale-layout
  hint is emitted.

## Acceptance Criteria

- [ ] The explore check group runs only when the project's selected harness
  configuration registers the `grove-explore` MCP server — not from the
  declared mode.
- [ ] Structural checks (config loads, legal mode, lock verify, registry and
  grammar cache, project languages) are unchanged.
- [ ] The harness-consistency check understands the two-registration layout:
  `mcp-llm` expects both server registrations, other modes expect no
  `grove-explore` registration, and a pre-split single-registration layout is
  flagged with a hint to re-run `grove init --as mcp-llm`.
- [ ] `grove doctor` no longer accepts `--explore` or `--standard`; the `--json`
  output shape and the exit-code gate are preserved.
- [ ] `cargo test --release --locked` passes.
- [ ] `cargo clippy --all-targets --workspace --locked -- -D warnings` passes.
- [ ] All modified files end with a newline.

## Operational Impact

- **Distribution:** rides the next release train; no immediate user action is
  required to install the new doctor behaviour.
- **Backwards compatibility:** existing `mcp-llm` projects that still have the
  pre-split single-registration layout will see a stale-layout warning or
  failure from `grove doctor`, guiding them to re-run `grove init --as mcp-llm`.
  Doctor itself continues to be read-only and does not modify files.
- **Regeneration:** none; doctor only gains the stale-layout hint.
- **Security scan:** not required.

## Known Doc Debt (out of scope)

- `docs/doctor-command-proposal.md` still documents `grove doctor [path]
  [--explore] [--standard]` and will go stale after the flag removal. It is
  outside this sprint's docs-restructure scope (item 9) and should be swept in
  a future doc pass.
