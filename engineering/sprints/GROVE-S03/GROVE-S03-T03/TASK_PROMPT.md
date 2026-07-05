# GROVE-S03-T03: `serve` reads declared mode + shared `active_mode` resolver

**Sprint:** GROVE-S03
**Estimate:** M
**Pipeline:** default

---

## Objective

Switch `grove serve`'s surface decision from the emergent
`explore.json.exists()` signal to the declared `config.mode`, fixing **bug 1**
(the sticky explore surface). Introduce a single `active_mode(root, force)`
resolver that both `serve` and (later) `doctor` call, so they can never disagree
about which mode is active.

## Acceptance Criteria

1. A shared resolver — `active_mode(root, force) -> Mode` (in `core/`, e.g.
   `core::config`) — resolves the active integration mode from `GroveConfig`
   plus a `force` choice (none / force-explore / force-standard), applying the
   same precedence `serve` uses today (`--standard` wins over `--explore`).
2. `determine_surface` (`cli/src/mcp.rs:43`) is rewritten to key off
   `active_mode(...) == Mode::McpLlm` instead of
   `ExploreConfig::config_path(root).exists()`. It loads the explore section
   from `GroveConfig` (not `ExploreConfig::load`).
3. After `init --as mcp` sets `mode: "mcp"`, a subsequent `serve` boots the
   **standard** 7-tool surface immediately, even if a stale `.grove/explore.json`
   or leftover explore config still exists (bug 1 regression test).
4. With `mode: "mcp-llm"` and a healthy provider, `serve` surfaces explore-only;
   the S02 **health-gated fallback** (mcp-llm + unhealthy provider → standard
   surface, with the stderr note) is preserved unchanged.
5. `--explore` / `--standard` runtime overrides still work with identical
   precedence to today.
6. Existing MCP smoke test and `serve`/surface tests pass (updated only where
   they must reflect the `config.json` trigger); `cargo build` warning-clean,
   `cargo clippy -- -D warnings` clean, `cargo test` green.

## Context

Implements item 3 of `SPRINT_REQUIREMENTS.md` and ADR 0002 §Decision-1. Current
code: `cli/src/mcp.rs` `determine_surface` (reads
`ExploreConfig::config_path(root).exists()` then `health_probe`). Depends on
**T01** (`GroveConfig`, `Mode`) and **T02** (migration — so a legacy `mcp-llm`
project resolves to `Mode::McpLlm` rather than falling to Standard). The
`active_mode` helper is deliberately shared to prevent `serve`/`doctor`
divergence (a named risk).

## Artifacts Involved

- `core/src/config.rs` — `active_mode(root, force)` + a small `ModeChoice`/force
  enum.
- `cli/src/mcp.rs` — `determine_surface` rewrite; `Surface::Explore` now sourced
  from `GroveConfig.explore`.

## Operational Impact

- **Version bump:** required at release (changes which surface `serve` boots for
  existing projects — this is the bug-1 fix).
- **Regeneration:** none; behaviour change is automatic once `config.json` is
  present (via T02 migration).
- **Security scan:** not required.
