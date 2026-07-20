# PLAN REVIEW — GROVE-S04-T07: doctor re-keying — explore checks keyed on grove-explore registration, not mode

(standalone review)

## Review Summary

The revised plan resolves both blocking items raised in the prior review.

**Required Change #1 (resolved):** §4 now takes the *conservative* deletion path
the prior review offered as the clippy-clean alternative — drop only the
`ForceExplore` / `ForceStandard` variants from `ModeChoice`, keep
`ModeChoice::None` as the sole variant and keep
`active_mode(root, ModeChoice) -> Mode` (single match arm). "Files to Modify"
lists only `core/src/doctor.rs`, `cli/src/main.rs`, `core/src/config.rs`, and the
plan adds an explicit "intentionally not modified" section naming
`core/src/lib.rs` (re-export of `active_mode`/`ModeChoice` at line 68) and
`cli/src/init.rs` (test `reconcile_harness_then_save_config_active_mode` calling
`active_mode(&dir, ModeChoice::None)` at lines 1602/1612) as legitimately
preserved. I independently verified the conservative path is clippy-clean: a
single-variant enum matched with a single *named* arm (`ModeChoice::None => …`)
does **not** trip `clippy::match_single_binding` under `clippy --all-targets --
-D warnings` (I tested this empirically in an isolated crate; the lint only
fires on wildcard `_` arms). Both unlisted files remain valid under this path,
so the prior compile-failure risk is gone and the plan is no longer
self-contradictory.

**Required Change #2 (resolved):** §5 and the Testing Strategy now explicitly
name the previously-missing fixture: "a new multi-harness / non-Claude
`mcp-llm` fixture that asserts the explore group runs when `grove-explore` is
registered only in Cursor (or another non-Claude harness), and that drift
detection still passes or fails per harness." This closes the verification loop
on the harness-agnostic re-keying that was the prior blocking item #1.

## Feasibility

Verified directly against the source. The building blocks the plan relies on
all exist and are tested:

- `harness::EXPLORE_SERVER_KEY = "grove-explore"` and
  `harness::expected_explore_args(mode)` (returning `Some(&[])` only for
  `McpLlm`) — present in `core/src/harness.rs`.
- `harness::expected_mcp_args(mode)` and `MCP_SERVER_KEY = "grove"` — present.
- `check_harness_registration(root, home, h, mode)` and `read_grove_args(path,
  format)` — present in `core/src/doctor.rs`; mirroring them for the
  `grove-explore` entry is a direct, low-risk extension.
- `McpFormat` covers all six harnesses: ClaudeCode/Cursor/Gemini/Windsurf use
  `Json { root_key: "mcpServers" }`, VS Code uses `Json { root_key: "servers" }`,
  Codex uses `Toml { table: "mcp_servers" }`. `mcp_config_path_in(root, home)`
  resolves each path. A `grove-explore` registration-detection helper iterating
  the configured harness set is therefore feasible across every format the plan
  names in §1 — no harness is left uncovered.
- The current explore trigger is a single `if mode == Mode::McpLlm` arm at the
  tail of `diagnose`; re-keying it to `if has_explore_registration` is a
  one-line gate change. `check_harness_serve_surface(mode, has_explore_cfg)`
  already accepts a `has_explore_cfg` boolean, so repointing its second argument
  to registration presence is mechanical.

No new crate dependency is introduced. `core/Cargo.toml` (crate `grove-cst`)
has **no** `grove-explore-core` dependency; `cli/Cargo.toml` already depends on
it but the plan adds nothing there. Crate layering is preserved, satisfying the
stack-checklist layering rule.

## Spec Compliance

Acceptance criteria mapped:

1. Explore group gated on `grove-explore` registration, not mode — addressed by
   §2's re-keying. The plan widens the trigger beyond the literal AC #1 text
   (".mcp.json registers") to *any selected harness*, which is the
   harness-agnostic direction the prior review (and T04's two-registration
   layout across harnesses) established. Consistent with the corrected reading.
2. Structural checks unchanged — preserved; only `harness_serve_surface`'s
   *reporting* (an Info check, not in AC #2's structural list) is repointed to
   registration presence. Status remains Info.
3. Two-registration drift matrix + stale-layout hint — §3's three-row matrix
   (mcp-llm missing explore → fail; non-mcp-llm unexpected explore → fail;
   pre-split `grove --explore` → stale-layout hint) covers the cases. The
   stale-layout check keys on the literal `--explore` arg inside any `grove`
   entry, which is the correct pre-split marker (a `grove --standard` entry
   just yields a generic args-mismatch fail, which is acceptable).
4. `--explore`/`--standard` removed from `grove doctor`, `--json` shape and
   exit-code gate preserved — §4 deletes the flags and force plumbing in the
   `Cmd::Doctor` branch (the only `ModeChoice` reference site in `main.rs`),
   and `diagnose` drops its `force` parameter. Doctor deletes the flags
   outright (no error-guard) because, unlike `grove serve`, it has no successor
   binary to redirect to — §4 states this.
5. Workspace green — Testing Strategy lists `cargo test --release --locked`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`, warning-clean
   build, newline-terminated files. Matches the project's CI gate.

## Plugin Impact Assessment

- **Version bump declared?** Yes — flagged material (explore gating + new
  stale-layout hint). Correct.
- **Migration entry?** N/A — doctor is read-only (reports/hints).
- **Security scan?** Correctly not required.
- **Schema change?** No `.forge/store/` or `.forge/config.json` schema change.

## Security

No concerns. Doctor remains read-only; the new detection only reads on-disk
harness config files the doctor already reads for the structural `grove` entry.
No new untrusted input is parsed.

## Testing Strategy

Adequate. Unit-test matrix covers registration-gated group selection, the
named non-Claude/multi-harness `mcp-llm` fixture, stale-layout detection, and
both two-registration drift rows. The existing
`multi_harness_config_checks_each_registration` and
`multi_harness_missing_cursor_registration_is_fail` fixtures (already in
`core/src/doctor.rs`) provide the seeding pattern for the new fixture.
Integration tests assert `grove doctor --help` no longer documents the flags and
that exit-code/`--json` shape are preserved.

## Verdict: Approved

## Advisory Notes (non-blocking)

1. **Flag-removal mechanism vs T03 (carried forward).** §4 deletes `doctor
   --explore`/`--standard` outright with no error-guard, whereas T03 kept
   `serve --explore`/`--standard` as hidden error-guards redirecting to
   `grove-explore serve` (verified: `cli/tests/cli.rs` `serve_removed_explore_flag_errors_with_hint`).
   The different mechanism is justified (doctor has no successor command to
   redirect to) and §4 now states this explicitly. The rationale line
   "consistency with T03" still lightly overstates the similarity — the
   mechanism is opposite (error-guard vs. complete deletion) — but the
   one-line clarification in §4 is sufficient for the engineer not to mirror
   T03's error-guard pattern here. No change required.

2. **Stale proposal doc (carried forward).** `docs/doctor-command-proposal.md`
   documents `grove doctor [path] [--explore] [--standard]` and will go stale
   after the flag removal. Out of this sprint's docs-restructure scope (item 9);
   note for a future doc sweep. The plan records this as "Known Doc Debt (out of
   scope)."