# ARCHITECT_APPROVAL — GROVE-S04-T03

**Verdict:** Approved

## Scope Approved

Deletion of all bimodal `Surface` / `determine_surface` / health-probe fallback
machinery from `cli/src/mcp.rs`, removal of the `serve --explore` / `--standard`
flags (replaced by a clear replacement-naming error), and the guarantee that
`grove serve` unconditionally dispatches the 7-tool structural surface regardless
of declared mode, config contents, or provider health.

## Architectural Rationale

- **Coherence with the stack philosophy.** The MCP server is a hand-rolled,
  synchronous, single-purpose stdio JSON-RPC server (per `architecture/stack.md`).
  Collapsing the runtime-branching `Surface` enum + health-probe fallback into a
  single constant dispatch removes an entire axis of runtime state and moves the
  serve path toward the stack's stated no-async, single-responsibility design.
- **Reduced blast radius.** `handle()`/`serve()` signatures lose their
  `surface`/`trace` parameters; the collapse is contained — only `main.rs:405`
  called `mcp::serve`, and that site was updated. No downstream module depends on
  the removed symbols (verified: `grove_explore_core` import block was consumed
  solely by deleted code; `active_mode`/`Mode`/`ModeChoice` remain in core for
  `Cmd::Doctor`).
- **Independent verification.** `grep -nE "explore|Surface|determine_surface"
  cli/src/mcp.rs` returns zero hits. `cargo tree -i ratatui` confirms the AC4
  state below.

## Cross-Cutting Concerns

- **AC4 (ratatui/crossterm dep prune) is deferred to T05 — sanctioned.**
  `cargo tree -i ratatui` still shows `grove-cst-cli` as the consumer because
  `trace_tui/` and `config_tui/` retain them. A Cargo.toml prune in this task
  would break the build. This deferral is recorded in the approved PLAN and
  PLAN_REVIEW. T03 delivers the serve-path decoupling only; `mcp::serve` itself
  carries zero ratatui/crossterm imports. **The joint T03+T05 `cargo tree` gate
  must be run and pass when T05 lands the TUI move + Cargo.toml prune** — do not
  let it fall through the cracks.
- **T04 sequencing.** Existing `mcp-llm` projects keep working only after T04's
  migration. The release must ship T03+T04 together; the intermediate state
  (flags removed, migration absent) must never ship alone.

## Deployment Notes

- **Version bump:** rides the next release train.
- **CHANGELOG:** the removed `--explore` / `--standard` flags need a deprecation
  note pointing at the `grove-explore` / plain `grove serve` replacements.
- **Security scan:** not required.
- No migrations, no schema changes, no runtime config changes beyond the flag
  removal (which errors loudly rather than silently ignoring).

## Follow-Up Items For Future Sprints

1. **T05 (this sprint):** land the TUI move + Cargo.toml prune, then run the
   AC4 `cargo tree -i ratatui` / `cargo tree -i crossterm` gate to completion.
2. **T04 (this sprint):** ship the `mcp-llm` migration in the same release as T03.
3. **Release engineering:** add the CHANGELOG deprecation entry for the removed
   flags before the release train departs.
