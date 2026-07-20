# Architect Approval — GROVE-S04-T06

## Task
`init --as mcp-llm` funnel — shell-out to `grove-explore config` + graceful PATH degrade.

## Architectural Review

### Alignment with project architecture
The implementation completes the sprint's two-binary split story (T01–T05
extracted `grove-explore`; T06 wires `init` to delegate to it). This is the
correct architectural posture: `grove init` is the one-shot provisioning
entry point, and the first-run configuration TUI now lives in the sibling
that owns it (`grove-explore config`). The 4-cell decision matrix in `run()`
(PATH × TTY) is the right factoring — presence and interactivity are
orthogonal concerns and are probed independently.

The presence/writer split is clean and worth calling out as the load-bearing
invariant:
- `explore_binary_on_path() -> Option<PathBuf>` — guard and first-run branch
  decision (presence question).
- `explore_command_value() -> PathBuf` — always succeeds, falls back to the
  bare string `grove-explore` (command-value question).

Both `write_json_mcp_explore_server` and `write_toml_mcp_explore_server`
consume `explore_command_value()`, so `reconcile_harness_for` completes
during the degrade path instead of erroring. This is what makes graceful
degrade actually work: the harness registrations are written with a
resolvable-at-runtime command name, and the user is told how to finish setup.

### Cross-cutting concerns
- **No schema changes.** `.forge/store/` and `.forge/config.json` are
  untouched; `GroveConfig` persistence is unchanged from T04.
- **Non-mcp-llm modes isolated.** The guard, shell-out, and degrade block
  are all gated on `target == Target::McpLlm`; `init --as mcp|skill|both|grammars`
  behavior is byte-identical to T04. Confirmed by the 43 existing cli
  integration tests passing unchanged.
- **`main.rs` retains its own `find_explore_binary`** for the deprecated
  `config`/`tap` shims (T05). The removal of the init.rs copy is consistent
  — init no longer needs the sibling-directory lookup because it probes PATH.
- **Dry-run isolation preserved.** `--dry-run` returns before the first-run
  block and before reconcile; no shell-out, no writes, previews only.

### Operational impact
- **Distribution:** ships on the next release train. Full installs (both
  binaries on PATH) get the same first-run UX via `grove-explore config`.
  Minimal installs that only place `grove` on PATH degrade gracefully
  instead of failing — the intended funnel improvement.
- **Backwards compatibility:** existing project files remain valid;
  re-running `init` reconciles forward as before. No migration entry.
- **Security:** shell-out target is a fixed sibling binary name
  (`grove-explore`) with no user-controlled arguments beyond the project
  root path. No scan required.

### Independent verification
- `cargo test --release --locked` → 341 passed, 0 failed (re-run).
- `cargo clippy --all-targets --workspace --locked -- -D warnings` →
  warning-free (re-run).
- `cli/src/init.rs` and `cli/tests/cli.rs` both end with a newline.
- Both PATH-controlled integration tests are deterministic: the absent-path
  test uses an empty temp directory as the child's *entire* PATH (not a
  prepend), so `which::which("grove-explore")` cannot resolve regardless of
  the parent environment.

## Advisories (non-blocking, for future sprints)
1. **Duplicated bail message.** The interactive-terminal message is repeated
   verbatim between the top guard (init.rs:111) and the defensive re-check
   (init.rs:147). A `const` would prevent drift. Minor.
2. **Repeated `which::which` calls.** `explore_binary_on_path()` is invoked
   up to 4× per init run (guard + first-run block + 2 writer call sites via
   `explore_command_value`). Not a correctness issue for a one-shot CLI;
   caching the probe once in `run()` would be cleaner. Minor.

## Follow-up items for future sprints
- Extract the duplicated interactive-terminal message into a `const` when
  next touching `init.rs`.
- Consider caching the PATH probe result in `run()` and threading it through
  to the writers if init ever becomes hot or re-entrant.

**Verdict:** Approved