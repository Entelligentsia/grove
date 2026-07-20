# ARCHITECT APPROVAL — GROVE-S04-T07: `doctor` re-keying — explore checks keyed on `grove-explore` registration

**Verdict:** Approved

## Approval Rationale

GROVE-S04-T07 re-keys `grove doctor`'s explore check group from a mode-inference gate
(`mode == McpLlm`) to a harness-agnostic registration-presence gate (`has_explore_registration`
scanning all six harness configs), extends the harness-consistency drift matrix for the
two-registration (`grove` + `grove-explore`) layout and the stale single-registration
(`--explore`-bearing) layout, and removes the orphaned `--explore` / `--standard` force flags
from `Cmd::Doctor`. The change is consistent with the GROVE-S04-T04 redefinition of `mcp-llm`
as "register both servers" and with GROVE-S04-T03's removal of the same flags from `grove serve`.

### Architectural Alignment

- **Crate layering preserved.** `grove-cst` (core) gains no `grove-explore-core` dependency.
  The registration-detection helper reads harness config files via the existing
  `mcp_config_path_in` / `mcp_format` / `read_grove_args` primitives already shared across
  Claude, Cursor, Codex, Gemini, Windsurf, and VS Code. No new cross-crate boundary introduced.
- **Conservative API pruning.** Only `ModeChoice::ForceExplore` / `ForceStandard` were dropped;
  `ModeChoice::None` and `active_mode` are retained, leaving `core/src/lib.rs:68` and
  `cli/src/init.rs:1602/1612` untouched. All `active_mode` callers now pass `None`. This is the
  minimal-blast-radius path the approved plan §4 specified.
- **Harness-agnostic trigger.** Moving the explore gate from mode equality to registration
  presence future-proofs `doctor` against new harnesses: any selected harness registering
  `grove-explore` lights up the explore group, decoupling diagnostics from the mode enum.
- **Read-only semantics.** `doctor` only reports and hints; it performs no mutation, makes no
  network calls, and accepts no user-controlled path components. Malformed JSON/TOML is
  swallowed to `None` (no registration), so corrupted harness configs degrade gracefully.

### Spec Compliance (AC-by-AC)

| AC | Status | Evidence |
|----|--------|----------|
| AC1 — explore group iff `grove-explore` registered | ✓ | `has_explore_registration` scans all 6 harnesses; tests pin both gate sides incl. non-Claude Cursor fixture |
| AC2 — structural checks, `--json` shape, exit-code gate preserved | ✓ | byte-identical to pre-T07; `doctor_json_output_is_valid`, `doctor_fail_exits_nonzero` green |
| AC3 — drift matrix for two-registration + stale-layout | ✓ | per-mode validation of `grove` + `grove-explore`; stale `--explore` arg short-circuits to `grove init --as mcp-llm` hint |
| AC4 — `--explore`/`--standard` removed; `--json` shape + exit code preserved | ✓ | `Cmd::Doctor` pruned; `ModeChoice` → `None` only; `doctor_help_documents_verb` asserts flags absent from `--help` |
| AC5 — workspace green | ✓ | `cargo test --release --locked` 343 pass; `cargo clippy --all-targets --workspace --locked -- -D warnings` clean; all modified files end with newline |

### Cross-Cutting Concerns

- **No migration, no schema change.** doctor is read-only; `.forge/store/` and
  `.forge/config.json` schemas are untouched.
- **Version bump flagged material** — `doctor` behaviour changes materially; rides the next
  release train (no immediate user action required).
- **Backwards compatibility.** Existing `mcp-llm` projects still carrying the pre-split single
  `grove` entry with `--explore` get a stale-layout hint to re-run `grove init --as mcp-llm`.

## Deployment Notes

- **Distribution:** rides the next release train; no immediate user action required.
- **Backwards compatibility:** stale `mcp-llm` layouts are detected and hinted, not silently
  broken — the doctor surfaces a re-init hint rather than a hard failure.
- **Regeneration:** none; doctor only gains the stale-layout hint.
- **Security scan:** not required (read-only, no network, no user-controlled paths).

## Follow-Up Items for Future Sprints

1. **Doc debt (out of scope, item 9):** `docs/doctor-command-proposal.md` will go stale after the
   `--explore` / `--standard` flag removal; should be refreshed in a docs-grooming pass.
2. **DRY refactor (advisory):** the pre-existing duplication between `check_harness_mcp_json`
   and `check_harness_registration` was extended consistently but not unified; a future
   refactor could collapse them into a single harness-config validator.
3. **Mode-blind stale hint (advisory):** the stale-layout hint always recommends
   `grove init --as mcp-llm` even when the project was downgraded to `mcp` mode. Plan-conformant
   for the rare edge case; a future enhancement could make the hint mode-aware.