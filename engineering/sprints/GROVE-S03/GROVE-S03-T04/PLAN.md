# Plan: GROVE-S03-T04 — `reconcile_harness` single harness writer + transition-matrix test

## Objective

Extract a single `reconcile_harness(root, old_mode: Option<Mode>, new_mode: Mode) -> Result<Vec<String>>`
function in `cli/src/init.rs` that owns the complete `mode → on-disk harness` mapping, adding a
**strip** direction (to remove old-mode residue) alongside the existing write direction. Update
`grove init` (`run`) to load the prior mode from `config.json`, call `reconcile_harness`, and
persist the new mode. Prove correctness with a table-driven transition-matrix test over all 20
ordered `A → B` mode switches.

---

## Approach

### 1. Analyse existing code paths

The current write path in `cli/src/init.rs`:

| Function | What it does |
|---|---|
| `write_harness(root, target)` | Dispatches per-mode to the three per-file writers; **no strip** |
| `write_steering_md(root, file, section, msg)` | Idempotently writes or replaces the sentinel block |
| `write_mcp_json_with(root, args, msg)` | Upserts grove's entry in `.mcp.json` |
| `write_mcp_json` / `write_mcp_json_explore` | Thin wrappers over `write_mcp_json_with` |
| `write_claude_md` / `write_agents_md` | Thin wrappers over `write_steering_md` |

Missing from all current paths: the **strip** direction — removing a sentinel block from a steering
file and removing grove's entry from `.mcp.json`.

### 2. New helper functions

**`strip_steering_block(root: &Path, filename: &str) -> Result<()>`**  
Reads `filename` (if it exists), removes the `<!-- grove:start -->…<!-- grove:end -->` sentinel block
(including surrounding blank lines added by `write_steering_md`), and writes the result back.
Host-authored content before or after the block is preserved verbatim. If the file is absent or
has no grove block, returns `Ok(())` with no write. If the block is the entirety of the file, the
file is left with empty content (not deleted — deletion would be a surprise to users who created
the file).

**`strip_grove_entry_from_mcp_json(root: &Path) -> Result<()>`**  
Reads `.mcp.json` (if it exists and is valid JSON), removes the `"grove"` key from
`mcpServers`, and writes the result back. Other servers are untouched. If the file is absent or
has no grove entry, returns `Ok(())`.

### 3. `Target::to_mode(self) -> Mode`

A method that maps each `Target` variant to its `grove_core::config::Mode` counterpart (1-to-1).
Used to bridge the CLI `Target` enum (clap-facing) and the store `Mode` enum (config-facing).

### 4. `reconcile_harness` function

Signature: `fn reconcile_harness(root: &Path, old_mode: Option<Mode>, new_mode: Mode) -> Result<Vec<String>>`

The function loads `langs` from `grove.lock` once, then reconciles each harness file toward
`new_mode` by consulting the three-file harness table:

| Mode | `.mcp.json` grove entry args | `CLAUDE.md` block | `AGENTS.md` block |
|---|---|---|---|
| `Mcp` | `["serve"]` | present (Mcp content) | absent |
| `Skill` | absent | present (Skill content) | absent |
| `Both` | `["serve"]` | present (Mcp content) | absent |
| `McpLlm` | `["serve", "--explore"]` | present (McpLlm content) | present (explore content) |
| `Grammars` | absent | absent | absent |

For each artifact:

- **`.mcp.json`**: if `new_mode ∈ {Mcp, Both}` → `write_mcp_json`; if `McpLlm` →
  `write_mcp_json_explore`; if `Skill` or `Grammars` → `strip_grove_entry_from_mcp_json`.
- **`CLAUDE.md`**: if `new_mode == Grammars` → `strip_steering_block("CLAUDE.md")`; else →
  `write_claude_md` with the mode-appropriate content (via `Target::to_mode → write_claude_md` with
  the matching `Target`).
- **`AGENTS.md`**: if `new_mode == McpLlm` → `write_agents_md`; else →
  `strip_steering_block("AGENTS.md")`.

The `old_mode` parameter is accepted but not needed for reconciliation logic (the function always
drives toward `new_mode` from on-disk state). It is kept in the signature per the AC so future
optimisations (e.g. skip-if-already-correct) can use it without a signature break.

Returned `Vec<String>` lists only the artifacts that were **written** (same as current
`write_harness`); stripped files are silently reconciled (no entry in the list).

`write_harness` is removed and its callers in `run` are replaced with `reconcile_harness`.

### 5. Update `run`

The updated `grove init` (`pub fn run`) will:

1. **Load old config** — `let old_cfg = GroveConfig::load(root).ok();` (returns `Option<GroveConfig>`).
   Extract `old_mode: Option<Mode>` from it.
2. **Non-TTY guard** — updated to treat an existing `config.json` with `mode: mcp-llm` as
   "already configured" (not just `explore.json` presence), so CI re-runs of McpLlm are never
   blocked once `config.json` is present.
3. **Provision grammars** — unchanged (calls `provision_project`).
4. **First-run TUI** — unchanged (fires when `Target::McpLlm` and no existing config).
5. **Reconcile harness** — `let mut wrote = reconcile_harness(root, old_mode, target.to_mode())?;`
6. **Save mode** — build and save the new `GroveConfig`:
   - For `McpLlm`: load `ExploreConfig::load(root).ok()` (TUI just saved it to `explore.json`) and
     set `explore: Some(cfg)`. If TUI was skipped (re-run), the existing explore config is
     preserved from `old_cfg`.
   - For other modes: preserve `explore` from `old_cfg` (so switching away from McpLlm and back
     doesn't lose the explore config).
   - `new_cfg.save(root)?;`
7. **Print results** — unchanged (the `wrote` list feeds the terminal output).

The `dry_run` path does not call `reconcile_harness` or `save` (unchanged behaviour).

### 6. Transition-matrix test

A single `#[test] fn reconcile_harness_transition_matrix()` in `mod tests`:

```
// Modes under test
let modes = [Mode::Mcp, Mode::Skill, Mode::Both, Mode::McpLlm, Mode::Grammars];

// 20 ordered (A, B) pairs where A ≠ B
for &old in &modes {
    for &new in &modes {
        if old == new { continue; }
        let dir = tmp(&format!("matrix_{old:?}_{new:?}"));
        seed_lock(&dir);

        // Seed the initial harness for mode A
        reconcile_harness(&dir, None, old).unwrap();

        // Transition to mode B
        reconcile_harness(&dir, Some(old), new).unwrap();

        // Assert harness is consistent with B
        assert_mcp_json_consistent(&dir, new);   // args match or entry absent
        assert_claude_md_consistent(&dir, new);  // block present / absent, content tag correct
        assert_agents_md_consistent(&dir, new);  // block present only for McpLlm
    }
}
```

Helper assertions:
- **`assert_mcp_json_consistent(dir, mode)`** — reads `.mcp.json` if present; for modes that use
  MCP checks args (`["serve"]` or `["serve","--explore"]`); for non-MCP modes checks grove entry
  is absent (key not present in `mcpServers`). File may not exist for Skill/Grammars — that's
  also valid (no file = no entry).
- **`assert_claude_md_consistent(dir, mode)`** — for Grammars: file either absent or has no grove
  block. For all others: grove block present and contains the correct mode-discriminating string
  (e.g. `"mcp__grove__explore"` for McpLlm, `"mcp__grove__outline"` for Mcp/Both, `"grove skill"`
  for Skill).
- **`assert_agents_md_consistent(dir, mode)`** — for McpLlm: grove block present. For all others:
  file absent or grove block absent.

The `active_mode` assertion (AC #5 mentions it) is deferred to an integration path; the
transition-matrix unit test focuses on the three harness artifacts which are under `reconcile_harness`'s direct control. A separate `reconcile_harness_then_save_config_active_mode` test will verify the full round-trip (reconcile + GroveConfig::save → active_mode returns correct Mode).

### 7. Host-content-preservation test

A `#[test] fn reconcile_harness_preserves_host_content()` test:
- Seeds `CLAUDE.md` with host-authored content, appends an Mcp block
- Transitions to Grammars mode → grove block stripped, host content intact
- Seeds `.mcp.json` with two servers (grove + other), transitions to Skill → grove entry removed,
  other server preserved
- Seeds `AGENTS.md` with host content + McpLlm block, transitions to Mcp → grove block stripped,
  host content intact

---

## Files to Modify

| File | Changes |
|---|---|
| `cli/src/init.rs` | Add imports; add `Target::to_mode`; add `strip_steering_block`, `strip_grove_entry_from_mcp_json`, `reconcile_harness`; update `run`; replace `write_harness` call; update/add unit tests |

No changes to `core/src/config.rs` — `GroveConfig`, `Mode`, `ModeChoice`, `active_mode`, and
`GroveConfig::save` are all already in place from T01/T03.

No changes to `cli/tests/cli.rs` — the integration test `init_provisions_and_wires_harness_per_target`
asserts only `.mcp.json`, `CLAUDE.md`, and `grove.lock` presence, which remains correct. The new
`.grove/config.json` write is additive and not checked by the existing assertion.

---

## Data Model Changes

None. `Mode`, `GroveConfig`, and `GroveConfig::save` already exist in `core/src/config.rs` (T01).

---

## Testing Strategy

### Unit tests (`cli/src/init.rs` `mod tests`)

| Test | Scope |
|---|---|
| `strip_steering_block_removes_block_leaves_host_content` | CLAUDE.md with host content before the block; host content preserved, block removed |
| `strip_steering_block_empties_file_when_block_only` | CLAUDE.md with only the grove block; file written empty |
| `strip_steering_block_noop_when_no_block` | CLAUDE.md without grove block; untouched |
| `strip_steering_block_noop_when_absent` | File doesn't exist; no error |
| `strip_grove_entry_from_mcp_json_removes_grove_key` | `.mcp.json` with grove + other; grove removed, other kept |
| `strip_grove_entry_from_mcp_json_noop_when_absent` | `.mcp.json` doesn't exist; no error |
| `strip_grove_entry_from_mcp_json_noop_when_no_grove_key` | `.mcp.json` with only other server; untouched |
| `reconcile_harness_transition_matrix` (20 pairs) | All A→B mode switches; harness artifacts consistent with B |
| `reconcile_harness_preserves_host_content` | Host content safety across all three files |
| `reconcile_harness_then_save_config_active_mode` | reconcile + GroveConfig::save → active_mode returns B |

Existing tests (`write_harness_*`, `grammars_target_*`, etc.) are updated to call
`reconcile_harness(dir, None, mode)` with `Mode::*` instead of `write_harness(dir, Target::*)`.
The `write_harness` private function is removed.

### Integration tests (`cli/tests/cli.rs`)

No new integration tests required (the unit transition-matrix test covers correctness at depth).
The existing `init_provisions_and_wires_harness_per_target` test must continue to pass.

### Build / lint

`cargo build --release --locked` warning-clean; `cargo clippy -- -D warnings` clean;
`cargo test --release --locked` green.

---

## Acceptance Criteria Mapping

| AC | Addressed by |
|---|---|
| 1. `reconcile_harness` signature and semantics | §4 + implementation |
| 2. `run` reads prior mode, calls reconcile, persists new mode | §5 |
| 3. Leaving mcp-llm strips AGENTS.md block + reverts .mcp.json args | §4 harness table + matrix test |
| 4. Only sentinel-delimited blocks and grove's own .mcp.json entry touched | `strip_steering_block` + `strip_grove_entry_from_mcp_json` design + host-content-preservation test |
| 5. Transition-matrix test over all 20 A→B pairs | §6 |
| 6. Existing tests pass | §7 "Existing tests updated" |
| 7. Build/clippy/test clean | §7 Build/lint |

---

## Operational Impact

- **Version bump**: required at release — `grove init` behaviour changes (reconciliation + strip).
- **Regeneration**: users switching modes get correct cleanup automatically; no manual steps.
- **Security scan**: not required.
- **Risk**: host-content safety. Mitigated by: `strip_steering_block` operates only on the
  sentinel-delimited region; `strip_grove_entry_from_mcp_json` removes only the `"grove"` key
  from `mcpServers`; both functions have explicit host-content-preservation tests.
