# GROVE-S03-T03 Plan — `serve` reads declared mode + shared `active_mode` resolver

## Objective

Replace `determine_surface`'s file-existence trigger (`explore.json.exists()`) with
a declared-configuration trigger (`config.mode == mcp-llm`) by introducing a single
`active_mode(root, force) -> Mode` resolver in `core::config` that both `serve` and
(later) `doctor` can call without risking divergence.

---

## Approach

### 1. Add `ModeChoice` and `active_mode` to `core/src/config.rs`

Introduce a small public enum `ModeChoice` (three variants: `None`, `ForceExplore`,
`ForceStandard`) and a public free function `active_mode(root: &Path, force: ModeChoice)
-> Mode`.

Precedence rules (matching today's `determine_surface` semantics):
1. `ModeChoice::ForceStandard` → always return `Mode::Mcp`, config file not read.
2. `ModeChoice::ForceExplore`  → always return `Mode::McpLlm`, config file not read.
3. `ModeChoice::None` → call `GroveConfig::load(root)` and return `cfg.mode`.
   On load failure (config absent and no legacy file) fall back to `Mode::Mcp` with
   a descriptive stderr diagnostic.

No network I/O, no health probe — this function is a pure config resolver.

### 2. Re-export `ModeChoice` and `active_mode` from `core/src/lib.rs`

Append to the existing `pub use config::{GroveConfig, Mode};` line so consumers can
name them as `grove_core::ModeChoice` / `grove_core::active_mode` without reaching
into the inner module.

### 3. Rewrite `determine_surface` in `cli/src/mcp.rs`

Replace the current logic with:

```
1. Map (force_standard, force_explore) → ModeChoice.
2. Call active_mode(root, force_choice).
3. If result != Mode::McpLlm  →  return Surface::Standard.
4. If result == Mode::McpLlm:
   a. Load GroveConfig::load(root) — may succeed (normal) or fail (edge case).
      On load failure → log diagnostic + return Surface::Standard.
   b. Extract grove_cfg.explore; if None → log diagnostic + return Surface::Standard.
   c. Call health_probe(&cfg) — same as today.
      On success → Surface::Explore { cfg, root }.
      On failure → log "explore provider unhealthy" + return Surface::Standard.
```

Key changes vs. today:
- `ExploreConfig::config_path(root).exists()` is gone — no more file-sniffing.
- `ExploreConfig::load(root)` inside `determine_surface` is gone — the explore
  section now comes from `grove_cfg.explore` (i.e. the config.json source of truth).
- `GroveConfig` loaded inside `determine_surface` is a second call after
  `active_mode` already loaded it; the redundancy is acceptable for clean separation
  of concerns. Both calls are cheap (one file read each).

Update imports: add `use grove_core::config::{active_mode, GroveConfig, ModeChoice, Mode};`.
The existing `use grove_core::explore::{..., ExploreConfig, ...}` import stays — 
`ExploreConfig` is still used for `Surface::Explore`, `explore_instructions`, etc.

### 4. Add unit tests in `core/src/config.rs`

Six new tests in the existing `#[cfg(test)] mod tests` block:

| Test name | Scenario | Expected |
|---|---|---|
| `active_mode_force_standard_returns_mcp` | `ForceStandard`, any config | `Mode::Mcp` |
| `active_mode_force_explore_returns_mcp_llm` | `ForceExplore`, any config | `Mode::McpLlm` |
| `active_mode_none_reads_declared_mcp_mode` | `None`, config has `mode: "mcp"` | `Mode::Mcp` |
| `active_mode_none_reads_declared_mcp_llm_mode` | `None`, config has `mode: "mcp-llm"` + explore | `Mode::McpLlm` |
| `active_mode_mcp_config_ignores_stale_explore_json` | `None`, config=mcp + stale explore.json | `Mode::Mcp` (bug-1 regression) |
| `active_mode_no_config_falls_back_to_mcp` | `None`, no config.json, no explore.json | `Mode::Mcp` (fallback) |

### 5. Update the existing integration test (if needed)

`explore_mode_unhealthy_provider_falls_back_to_standard_surface` in `cli/tests/cli.rs`
currently writes only `.grove/explore.json`. After T02's migration, `GroveConfig::load`
converts explore.json → config.json automatically, so `active_mode` correctly returns
`McpLlm` and the health probe still fails → standard surface. The test outcome is
unchanged, but the comment is updated to reference T03 as well as T02.

Additionally, a second integration test `bug1_serve_mcp_mode_ignores_stale_explore_json`
is added to `cli/tests/cli.rs` to exercise the bug-1 fix end-to-end:

- Write `.grove/config.json` with `mode: "mcp"` (no explore section).
- Write a stale `.grove/explore.json` with a plausible explore config.
- Spawn `grove serve`, send `initialize` + `tools/list`.
- Assert the response contains exactly 7 tools (standard surface), proving the
  stale explore.json is ignored.

---

## Files to Modify

| File | Change |
|---|---|
| `core/src/config.rs` | Add `ModeChoice` enum + `active_mode` fn + 6 unit tests |
| `core/src/lib.rs` | Re-export `ModeChoice` and `active_mode` |
| `cli/src/mcp.rs` | Rewrite `determine_surface`, update imports |
| `cli/tests/cli.rs` | Update comment on existing test; add `bug1_serve_mcp_mode_ignores_stale_explore_json` |

---

## Data Model Changes

None. `ModeChoice` is a transient enum used only as a function parameter — it is
never serialized or stored. `GroveConfig` and `Mode` are unchanged.

---

## Testing Strategy

### Unit tests (fast, offline)
- 6 new tests in `core/src/config.rs` cover all `active_mode` branches including
  the bug-1 scenario.
- Existing 261 tests must remain green.

### Integration tests (binary, offline)
- Existing `explore_mode_unhealthy_provider_falls_back_to_standard_surface` verifies
  the health-gated fallback path still works via migration.
- New `bug1_serve_mcp_mode_ignores_stale_explore_json` verifies the bug-1 fix
  end-to-end (config.mode=mcp + stale explore.json → 7-tool standard surface).

### Commands
```sh
cargo test --release --locked
cargo clippy -- -D warnings
```

---

## Acceptance Criteria Mapping

| AC | How satisfied |
|---|---|
| 1. `active_mode(root, force) -> Mode` exists in `core::config` | New function + enum in `core/src/config.rs` |
| 2. `determine_surface` keys off `active_mode == McpLlm` | `cli/src/mcp.rs` rewrite |
| 3. `mode: "mcp"` + stale explore.json → standard surface immediately | `active_mode_mcp_config_ignores_stale_explore_json` unit test + `bug1_serve_mcp_mode_ignores_stale_explore_json` integration test |
| 4. `mode: "mcp-llm"` + healthy provider → explore surface; health-gated fallback preserved | Existing test + existing code path retained in rewritten `determine_surface` |
| 5. `--explore` / `--standard` flags retain existing precedence | `active_mode` precedence rules match current `determine_surface` logic |
| 6. All existing tests green; clippy clean; release build clean | `cargo test && cargo clippy` in CI |

---

## Operational Impact

- **Version bump:** Required at release — this is a behavioural fix to `grove serve`
  (bug 1 resolution: surface selection now driven by declared config rather than
  file presence).
- **Migration path:** Fully covered by T02. Legacy `explore.json`-only projects are
  automatically migrated to `config.json` on first `GroveConfig::load` call; no
  manual steps required.
- **Backward compatibility:** Non-breaking. Projects without `config.json` get the
  same explore-mode behaviour as before (via migration). Projects with `config.json`
  get the correct declared-mode behaviour.
- **Security:** No change — no new I/O surfaces; `GroveConfig::load` is existing
  validated deserialization.
- **Regeneration:** None — behaviour change is automatic once `config.json` is present.
