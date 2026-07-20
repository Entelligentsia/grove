# PLAN — GROVE-S04-T06: `init --as mcp-llm` funnel — shell-out to `grove-explore config` + graceful PATH degrade

🌱 *grove Engineer*

**Task:** GROVE-S04-T06
**Sprint:** GROVE-S04
**Estimate:** M

---

## Objective

Keep the `grove init --as mcp-llm` adoption funnel working across the ADR 0004 split. The init flow must continue to register both MCP server identities and write the two-surface steering, but the first-run config TUI must now be reached by shelling out to the sibling `grove-explore config` binary. When that sibling is not on PATH, init must degrade gracefully: it still writes both server registrations, exits 0, and tells the user how to finish setup.

## Approach

1. **Separate PATH detection from command value.** Introduce a small helper in `cli/src/init.rs` that probes PATH for `grove-explore` using the already-declared `which` dependency. A second helper derives the MCP registration `command` field: the absolute PATH-resolved path when found, or the bare string `grove-explore` when absent. This mirrors the fallback used by `main.rs` for the deprecated `grove config` / `grove tap` shims.

2. **Make the first-run shell-out PATH- and TTY-aware.** Restructure the existing first-run guard so the PATH probe happens before the TTY decision. The action matrix becomes:

   | `grove-explore` on PATH | stdout is TTY | action |
   |---|---|---|
   | yes | yes | shell out to `grove-explore config <root>` |
   | yes | no | fail fast with the existing interactive-terminal message |
   | no | yes | degrade: register both servers, print setup message, exit 0 |
   | no | no | degrade: register both servers, print setup message, exit 0 |

   This preserves the non-TTY fast-fail for full installs (the sibling is present) while giving PATH-only installs a clean degrade path.

3. **Make the explore-server registration tolerate absence.** Update the two functions that write the `grove-explore` command into `.mcp.json` / `~/.codex/config.toml` to use the bare-name fallback instead of erroring when the sibling is missing. That lets `reconcile_harness_for` complete successfully during degrade, satisfying AC #3.

4. **Leave everything else unchanged.** `--dry-run` still performs no shell-out and no writes. Other init targets (`mcp`, `skill`, `both`, `grammars`) keep their current behavior. The `reconcile_harness` transition-matrix semantics from T04 are not modified.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `cli/src/init.rs` | Replace the sibling-directory lookup with a PATH probe plus a command-value fallback; restructure the first-run TUI block to be PATH- and TTY-conditional; update the explore-server JSON/TOML writers to use the fallback command value. | This is the entire funnel change: detection, shell-out, degrade, and registration writes. |
| `cli/tests/cli.rs` | Extend the `grove_mcp_llm` helper (or add a PATH-parameterized variant) so integration tests can run `grove init --as mcp-llm` with a controlled PATH; add a degrade-path test with `grove-explore` absent from PATH and a non-TTY fail-fast test with `grove-explore` present. | Required by AC #3 and AC #4; the degrade path must be exercised with explicit PATH isolation. |

## Plugin Impact Assessment

- **Version bump required?** Yes — `grove init` behavior changes materially (new shell-out path and degrade branch). It rides the next release train per the task prompt.
- **Migration entry required?** No — existing project files remain valid; re-running `init` reconciles them forward as before.
- **Security scan required?** No — the shell-out target is a fixed sibling binary name with no user-controlled arguments.
- **Schema change?** No — no `.forge/store/` or `.forge/config.json` schemas are touched.

## Testing Strategy

- **Workspace test suite:** run `cargo test --release --locked` after the implementation. This exercises the existing `init.rs` unit tests and the full `cli/tests/cli.rs` integration suite.
- **Lint gate:** run `cargo clippy --all-targets --workspace --locked -- -D warnings` to match CI.
- **New integration tests:**
  - *Degrade path (PATH-controlled):* run `grove init --as mcp-llm` with a PATH that excludes the `grove-explore` binary. Assert exit 0, assert both `grove` and `grove-explore` entries exist in `.mcp.json`, assert the `grove-explore` `command` is the bare string `grove-explore`, and assert the stdout contains the setup message.
  - *Non-TTY fast-fail (PATH-controlled):* run `grove init --as mcp-llm` with a PATH that includes the `grove-explore` binary but stdout piped (subprocess). Assert non-zero exit and an interactive-terminal diagnostic.
- **TTY shell-out limitation:** the actual on-PATH + interactive-terminal `grove-explore config` handoff cannot be exercised in the subprocess-based integration harness; it is covered by code review and manual smoke test only.

## Acceptance Criteria

- [ ] `init --as mcp-llm` registers both servers and writes the two-surface steering unchanged from T04.
- [ ] When `grove-explore` is on PATH and stdout is a TTY, `init` shells out to `grove-explore config <root>` for first-run setup.
- [ ] When `grove-explore` is on PATH but stdout is not a TTY, `init` fails fast with the existing interactive-terminal message.
- [ ] When `grove-explore` is not on PATH, `init` degrades gracefully: it still registers both servers, exits 0, and prints "run `grove-explore config` to finish setup".
- [ ] `--dry-run` performs no shell-out and no writes, and still previews the planned files.
- [ ] Existing `init --as mcp|skill|both|grammars` behavior is unchanged; existing `tests/cli.rs` init assertions continue to pass.
- [ ] `cargo test --release --locked` passes.
- [ ] `cargo clippy --all-targets --workspace --locked -- -D warnings` passes warning-free.
- [ ] All modified source files end with a newline.

## Operational Impact

- **Distribution:** the change ships with the next release train; both `grove` and `grove-explore` binaries are assumed present for full installs. No user action such as `/forge:update` is required.
- **Backwards compatibility:** full installs (both binaries present) see the same first-run UX via `grove-explore config`. Minimal installs that only put `grove` on PATH degrade gracefully instead of failing, which is the intended funnel improvement.
