# PLAN REVIEW — GROVE-S04-T06: `init --as mcp-llm` funnel — shell-out to `grove-explore config` + graceful PATH degrade

🌿 *grove Supervisor*

**Task:** GROVE-S04-T06  *(standalone review)*

---

**Verdict:** Approved

---

## Review Summary

The revised plan addresses both critical findings from the prior review. Finding #1 (registration-write call sites break the degrade path) is resolved by Approach §1 and §3, which introduce a command-value fallback helper and explicitly update the two explore-server write functions to use the bare-name fallback instead of erroring. Finding #2 (non-TTY guard must become PATH-conditional) is resolved by Approach §2, which states the PATH probe happens before the TTY decision and provides the complete 4-cell decision matrix. The approach is feasible, correctly scoped, and architecturally sound.

## Verification Performed (independent)

- Read `cli/src/init.rs::run` (line 78) — confirmed two separate McpLlm-specific
  blocks: the non-TTY guard (line ~107, fires before `provision_project`) and
  the first-run shell-out (line ~137, fires after `provision_project`). The plan's
  "restructure" directive covers both.
- Read `cli/src/init.rs::find_explore_binary` (line 607) — confirmed it returns
  `dir.join("grove-explore")` with no existence check; always `Ok`. The plan
  correctly replaces this with `which::which`-based detection.
- Read `cli/src/init.rs::write_json_mcp_explore_server` (line 617) and
  `write_toml_mcp_explore_server` (line 671) — confirmed both call
  `find_explore_binary()?` and write `exe.to_string_lossy()` as the `command`
  field. The plan's Approach §3 explicitly addresses these with the bare-name
  fallback.
- Read `cli/src/init.rs::reconcile_harness_for` (line 318) — confirmed that for
  McpLlm mode, `write_explore_harness_registration` (line 734) dispatches to the
  two write functions for every selected harness. This is the exact code path the
  degrade branch must execute.
- Read `cli/src/main.rs::find_explore_binary` (line 456) — confirmed it already
  falls back to bare `"grove-explore"` (PATH lookup by the agent at runtime). The
  plan's "mirrors main.rs" claim is accurate.
- Confirmed `which = "7"` in `cli/Cargo.toml` and `which::which` already used at
  `init.rs:284` for harness auto-detection — the "already-declared dependency"
  claim is correct.
- Read `cli/tests/cli.rs` — confirmed `grove_mcp_llm` helper (line 823) does not
  set PATH; existing tests pre-seed `explore.json` to bypass the non-TTY guard.
  The plan's PATH-parameterized variant is feasible.
- Confirmed `grove_explore_bin()` helper (line 58) builds `grove-explore-bin`
  into grove's `bin_dir` on-demand — a PATH including that dir satisfies
  `which::which`; excluding it does not. The testing strategy is executable.

## Feasibility

The approach is realistic and correctly scoped. It touches exactly two files
(`cli/src/init.rs` and `cli/tests/cli.rs`), which are the only files that need
changes. The separation of PATH detection (bool/Option) from command value
(String) is a clean design that avoids the prior review's critical error-path
issue: the write functions get a non-failing command value while the shell-out
block gets a reliable presence/absence decision. The 4-cell decision matrix
correctly implements AC #2–#4.

## Plugin Impact Assessment

- **Version bump declared correctly?** Yes — `grove init` behavior changes
  materially (new shell-out path and degrade branch); rides the next release
  train.
- **Migration entry targets correct?** N/A — existing project files remain
  valid; re-running `init` reconciles forward.
- **Security scan requirement acknowledged?** Yes — correctly noted as not
  required (shell-out target is a fixed binary name with no user-controlled
  arguments).

## Security

No new risks. The shell-out target is a fixed binary name (`grove-explore`)
with a single hardcoded argument (`config`) plus the project root path (already
trusted input). The bare-name fallback in MCP registrations writes
`"grove-explore"` as the command, which the MCP client resolves via PATH at
runtime — standard MCP registration pattern, no injection surface.

## Architecture Alignment

- The plan correctly separates detection (PATH probe) from command value
  (fallback string), mirroring the existing `main.rs` pattern.
- `--dry-run` bypass is preserved (existing `!dry_run` condition in the guard).
- Other init targets (`mcp`, `skill`, `both`, `grammars`) are untouched — they
  use different `Target` values and never enter the McpLlm-specific blocks.
- No schema changes (`additionalProperties: false` not applicable).

## Testing Strategy

The testing strategy is adequate and covers the two CI-testable paths:

1. **Degrade path** — PATH excludes `grove-explore`, asserts exit 0, both
   server entries in `.mcp.json`, bare `grove-explore` command value, and the
   setup message in stdout. Correct and complete.
2. **Non-TTY fast-fail** — PATH includes `grove-explore`, stdout piped, asserts
   non-zero exit and interactive-terminal diagnostic. Correct.

The TTY shell-out limitation (on-PATH + interactive terminal) is explicitly
acknowledged as not CI-testable, covered by code review and manual smoke test.
This is the correct call — subprocess stdout is always piped in the integration
harness.

`cargo test --release --locked` and `cargo clippy --all-targets --workspace
--locked -- -D warnings` are both specified, matching CI.

---

## If Approved

### Advisory Notes

1. **`already_configured` bypass in the restructured guard.** The plan's 4-cell
   matrix shows only the PATH × TTY dimensions. The existing guard includes
   `!already_configured` and `!dry_run` conditions that bypass the guard for
   re-runs and dry-runs. These must be preserved within the restructured guard.
   The plan's "restructure the existing first-run guard" directive implies this,
   but the engineer should ensure the PATH-conditional logic applies only when
   `!already_configured && !dry_run` — re-runs (where `old_mode == McpLlm` or
   `explore.json` exists) and dry-runs bypass the entire decision matrix.

2. **Two-block restructure.** The current code has the non-TTY guard *before*
   `provision_project` and the shell-out *after* it. The plan envisions a
   unified decision matrix but doesn't specify where the PATH probe stores its
   result for use by the post-provisioning shell-out/degrade block. The engineer
   should either (a) probe PATH early and store the result for later use, or (b)
   restructure so both decisions occur at the same point. Either works; the key
   invariant is that the degrade path runs `provision_project` then
   `reconcile_harness_for` then prints the message and exits 0.

3. **Degrade output flow.** The plan says "register both servers, print setup
   message, exit 0." The engineer should decide whether the degrade message
   replaces the normal "ready" output or supplements it. Either is acceptable;
   the message "run `grove-explore config` to finish setup" should be prominent
   and unambiguous.

4. **Re-run with absent explore.json.** When `already_configured=true` (prior
   McpLlm mode) but `explore.json` is still absent (user hasn't run
   `grove-explore config` yet), the shell-out block currently fires
   unconditionally. With the new PATH-conditional logic, this re-run should also
   degrade gracefully if PATH=no. This is consistent with the plan's approach but
   worth confirming during implementation.