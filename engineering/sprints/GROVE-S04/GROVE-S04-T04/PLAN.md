# PLAN — GROVE-S04-T04: `mcp-llm` = register-both

## Objective

Re-key `Mode::McpLlm` from single-explore to both-servers: `grove init --as
mcp-llm` registers the structural `grove serve` server **and** the
`grove-explore` MCP server in every agent's registration file, writes
dual-surface steering in `CLAUDE.md` / `AGENTS.md`, migrates pre-split
projects forward, and extends the S03-T04 transition-matrix test to cover
the new `mcp-llm` column correctly.

---

## Background

- **T02** added the `grove-explore` binary (binary name `grove-explore`,
  `[[bin]]` in `cli/Cargo.toml`); it is an MCP stdio server exposing a
  single `explore` tool under server key `grove-explore` → tool prefix
  `mcp__grove-explore__`.
- **T03** deleted `--explore` / `--standard` from `grove serve`; the structural
  server is now unconditional.
- Pre-T04 `mcp-llm` registered one entry: `grove` key, args `["serve",
  "--explore"]`. That argument no longer exists. The upgrade must:
  1. Replace `["serve", "--explore"]` with `["serve"]` in the `grove` key.
  2. Add a `grove-explore` key pointing to the `grove-explore` binary.

### Pre-existing functions to address

`fn write_mcp_json_explore` (cli/src/init.rs:624) is a **private production
helper** that hard-codes the now-invalid `["serve", "--explore"]` arg and has
**exactly two callers, both in tests** (`write_mcp_json_explore_registers_with_explore_flag`
at line 1493 and `write_mcp_json_explore_preserves_other_servers` at line 1509).
It is never called from production code (`reconcile_harness_for` uses
`write_harness_registration`). Once those two test callers are migrated to the
new `write_json_mcp_explore_server` helper, `write_mcp_json_explore` has zero
callers and is flagged `dead_code` by the linter — breaking AC7. It must be
deleted (or repurposed as the body of the new helper, described in §2c).

---

## Approach

### 1. `core/src/harness.rs` — per-mode harness shape constants

#### 1a. Add `EXPLORE_SERVER_KEY`
```rust
pub const EXPLORE_SERVER_KEY: &str = "grove-explore";
```
Used as the JSON key / TOML table name for the explore-server entry.

#### 1b. Fix `expected_mcp_args` for `McpLlm`
```rust
Mode::McpLlm => Some(&["serve"]),   // was ["serve", "--explore"]
```
The structural `grove` registration for McpLlm now takes the same args as
`Mcp`/`Both`. The explore-server is handled by the new `expected_explore_args`.

#### 1c. Add `expected_explore_args`
```rust
pub fn expected_explore_args(mode: Mode) -> Option<&'static [&'static str]> {
    match mode {
        Mode::McpLlm => Some(&[]),   // grove-explore binary takes no args
        _ => None,
    }
}
```
Returns `Some(&[])` only for `McpLlm`; all other modes strip the explore entry.

#### 1d. Update `expected_claude_marker` for `McpLlm`
```rust
Mode::McpLlm => Some("mcp__grove-explore__explore"),
```
The explore tool is now in the `grove-explore` server, not the `grove` server.

#### 1e. Update unit tests in `harness.rs`
- `expected_mcp_args_coverage`: change `McpLlm` assertion from
  `["serve", "--explore"]` → `["serve"]`.
- `expected_claude_marker_coverage`: change `McpLlm` assertion from
  `"mcp__grove__explore"` → `"mcp__grove-explore__explore"`.
- Add `expected_explore_args_coverage` covering all five modes.

---

### 2. `cli/src/init.rs` — reconcile logic and steering content

#### 2a. Import `EXPLORE_SERVER_KEY`
Add `EXPLORE_SERVER_KEY` to the import from `grove_core::harness`.

#### 2b. Add `fn find_explore_binary() -> Result<PathBuf>`
Locate `grove-explore` as a sibling of the running binary:
```rust
fn find_explore_binary() -> Result<PathBuf> {
    let grove = std::env::current_exe().context("locating grove binary")?;
    let dir = grove.parent().context("grove binary has no parent dir")?;
    Ok(dir.join("grove-explore"))
}
```
This binary is Unix-only for T04 scope; no `.exe` suffix is added. Document
this assumption in a code comment.

#### 2c. Add `fn write_json_mcp_explore_server(path, root_key, needs_type_stdio) -> Result<()>` and delete `write_mcp_json_explore`
Repurpose `write_mcp_json_explore` as the body of the new helper:
- Rename `write_mcp_json_explore` → `write_json_mcp_explore_server`, change its
  signature to `fn write_json_mcp_explore_server(path: &Path, root_key: &str, needs_type_stdio: bool) -> Result<()>`.
- Change its body to call `find_explore_binary()` and write the `grove-explore`
  entry under `EXPLORE_SERVER_KEY` with `args: []` (removing the old
  `["serve", "--explore"]` hard-coding). Preserve the rest of the file
  (same merge pattern as `write_json_mcp`).

```rust
fn write_json_mcp_explore_server(
    path: &Path, root_key: &str, needs_type_stdio: bool
) -> Result<()> {
    let mut doc: Value = /* read or default {} */;
    let exe = find_explore_binary()?;
    let mut entry = json!({ "command": exe.to_string_lossy(), "args": [] });
    if needs_type_stdio { entry["type"] = json!("stdio"); }
    doc[root_key][EXPLORE_SERVER_KEY] = entry;
    /* create_dir_all + atomic write */
    Ok(())
}
```

By renaming (not adding a sibling), `write_mcp_json_explore` is eliminated and
the `dead_code` lint is avoided — AC7 remains green.

#### 2d. Add `fn strip_json_mcp_explore_server(path, root_key) -> Result<()>`
Removes the `EXPLORE_SERVER_KEY` key from the JSON registration file;
no-ops when file / key absent.

#### 2e. Add `fn write_toml_mcp_explore_server(path, table) -> Result<()>`
For TOML harnesses (Codex): adds `[<table>.grove-explore]` entry.

#### 2f. Add `fn strip_toml_mcp_explore_server(path, table) -> Result<()>`
Removes `[<table>.grove-explore]` from TOML config; no-op when absent.

#### 2g. Add `fn write_explore_harness_registration(root, home, h) -> Result<String>`
Dispatch to JSON or TOML variant based on `h.mcp_format()`:
```rust
fn write_explore_harness_registration(
    root: &Path, home: &Path, h: HarnessId
) -> Result<String> {
    let path = h.mcp_config_path_in(root, Some(home));
    match h.mcp_format() {
        McpFormat::Json { root_key, needs_type_stdio } =>
            write_json_mcp_explore_server(&path, root_key, needs_type_stdio)?,
        McpFormat::Toml { table } =>
            write_toml_mcp_explore_server(&path, table)?,
    }
    Ok(format!(
        "{} ({} explore-server registration)",
        display_path(root, &path), h.display_name()
    ))
}
```

#### 2h. Add `fn strip_explore_harness_registration(root, home, h) -> Result<()>`
Dispatch to JSON or TOML strip variant.

#### 2i. Extend `reconcile_harness_for`
After the existing structural registration loop, add a second loop for the
explore server:

```rust
// ── grove-explore registration (McpLlm only) ─────────────────────────────
let wants_explore = harness::expected_explore_args(new_mode).is_some();
for &h in HarnessId::ALL {
    if wants_explore && harnesses.contains(&h) {
        wrote.push(write_explore_harness_registration(root, home, h)?);
    } else {
        strip_explore_harness_registration(root, home, h)?;
    }
}
```

This satisfies AC2 (single `reconcile_harness` writer) and the stripping
half ensures transitions away from `mcp-llm` remove the explore entry.

#### 2j. Update `harness_targets` (dry-run preview)
When `harness::expected_explore_args(mode).is_some()`, add a line per harness:
```
".mcp.json — grove-explore registration"
```

#### 2k. Rewrite `claude_section(Target::McpLlm)` — dual-surface steering

New content (locator-framed, recommended flow, names both server prefixes):

```
## Code navigation: grove — structural + explore surfaces

**grove** is configured with **two** MCP servers (languages: {langs}):

- **`mcp__grove__*`** — 7-tool structural surface (byte-precise, token-cheap):
  `mcp__grove__outline`, `mcp__grove__symbols`, `mcp__grove__source`,
  `mcp__grove__callers`, `mcp__grove__definition`, `mcp__grove__map`, `mcp__grove__check`.
  Use **directly** for precision work: read one symbol's body, map a directory,
  check syntax after an edit.

- **`mcp__grove-explore__explore`** — LLM-backed code locator: sweeps
  multiple files with tree-sitter + text tools, returns file:line citations.
  Use for **broad "where is X"** questions before you know which file to look in.

**Recommended flow to understand a feature:**
(1) `mcp__grove-explore__explore` — one narrow question per call
    ("where is X defined", "which files handle Y"); iterate broad → specific.
(2) `mcp__grove__source` / `mcp__grove__map` on the cited file:line locations.
(3) Synthesise the explanation yourself.

Do not delegate large multi-part tasks to `explore`; keep each question
single-focus. For text, configs, and non-code files, use the shell (`grep`, `rg`).
```

The new content must contain `"mcp__grove-explore__explore"` to satisfy
`expected_claude_marker(McpLlm)` in the doctor.

#### 2l. Rewrite `agents_section(Target::McpLlm)` — dual-surface steering

Mirrors CLAUDE.md content but without `mcp__grove__*` prefixes (AGENTS.md is
cross-agent harness-neutral). References `explore` as a tool name and
`outline`/`source`/etc. as the structural tools; the agent constructs the
prefixed name if needed. The block continues to satisfy
`agents_md_expected(McpLlm) == true`.

---

### 3. Forward Migration (AC3)

The migration happens naturally through the reconcile path:
- Calling `reconcile_harness(dir, old_mode, Mode::McpLlm)` reads the
  existing `.mcp.json`, writes the `grove` key with `["serve"]` (overwriting
  any prior `["serve", "--explore"]`), then adds the `grove-explore` entry.
- No pre-migration detection step is needed; `write_json_mcp` overwrites
  whichever args were there before.

A dedicated migration test confirms this:
**`mcp_llm_forward_migration_converges_single_to_two_registrations`**
- Seed `.mcp.json` with `{"mcpServers":{"grove":{"command":"...","args":["serve","--explore"]}}}`
- Call `reconcile_harness(&dir, None, Mode::McpLlm)`
- Assert `grove` key args == `["serve"]`
- Assert `grove-explore` key is present
- Assert old `["serve", "--explore"]` arg is gone

---

### 4. Test Changes — `cli/src/init.rs` unit tests

#### 4a. Update `assert_mcp_json_consistent` — all three branches

The McpLlm branch currently asserts `["serve", "--explore"]` (the old args)
and does not check the `grove-explore` key. The Mcp/Both and Skill/Grammars
branches do not inspect `grove-explore`. All three must be updated:

```rust
Mode::McpLlm => {
    // grove key → ["serve"] (was ["serve", "--explore"])
    assert_eq!(
        doc["mcpServers"]["grove"]["args"],
        json!(["serve"]),
        "mode McpLlm: .mcp.json grove args should be [serve]"
    );
    // grove-explore key must be present
    assert!(
        !doc["mcpServers"]["grove-explore"].is_null(),
        "mode McpLlm: grove-explore entry must be present in .mcp.json"
    );
}
Mode::Mcp | Mode::Both => {
    assert_eq!(
        doc["mcpServers"]["grove"]["args"],
        json!(["serve"]),
        "mode {mode:?}: .mcp.json args should be [serve]"
    );
    // grove-explore must NOT be present (only McpLlm registers it)
    assert!(
        doc.get("mcpServers")
            .and_then(|s| s.get("grove-explore"))
            .is_none(),
        "mode {mode:?}: grove-explore entry must be absent in .mcp.json"
    );
}
Mode::Skill | Mode::Grammars => {
    if path.exists() {
        // grove entry must be absent
        assert!(
            doc.get("mcpServers").and_then(|s| s.get("grove")).is_none(),
            "mode {mode:?}: grove entry should be absent in .mcp.json"
        );
        // grove-explore entry must also be absent
        assert!(
            doc.get("mcpServers").and_then(|s| s.get("grove-explore")).is_none(),
            "mode {mode:?}: grove-explore entry should be absent in .mcp.json"
        );
    }
}
```

This ensures the full transition matrix (including McpLlm→Mcp and McpLlm→Skill
transitions) verifies that `grove-explore` is stripped, satisfying AC4 and AC2.

#### 4b. `assert_claude_md_consistent` — McpLlm branch
Change expected marker from `"mcp__grove__explore"` to
`"mcp__grove-explore__explore"`.

#### 4c. `reconcile_harness_mcp_llm_writes_mcp_json_explore_and_steering`
- Assert `grove` args == `["serve"]` (not `["serve", "--explore"]`)
- Assert `grove-explore` key present
- Update `wrote.len()` from 3 → 4 (structural entry + explore entry + CLAUDE.md + AGENTS.md)

#### 4d. Rename and update `write_mcp_json_explore_registers_with_explore_flag`
New name: `write_explore_server_registers_in_grove_explore_key`.
- Now calls `write_json_mcp_explore_server` (the renamed helper) directly,
  or calls `write_explore_harness_registration` for ClaudeCode harness.
- Asserts `grove-explore` key present with `args == []`
- Asserts `grove` key is NOT affected (has `["serve"]` if pre-seeded)

#### 4e. Rename and update `write_mcp_json_explore_preserves_other_servers`
New name: `write_explore_server_preserves_other_servers`.
- Now calls `write_json_mcp_explore_server` (the renamed helper).
- Seed an existing `other` server
- Assert `other` server preserved and `grove-explore` key added with `args == []`

#### 4f. Update `claude_section_mcp_llm_routes_explore_not_individual_tools`
- Assert `s.contains("mcp__grove-explore__explore")`
- Assert `s.contains("mcp__grove__map")` (structural tools named)
- Assert `!s.contains("mcp__grove__explore")` (old single-server tool name gone)
- Remove or update the `s.contains("fallback")` assertion (automatic fallback
  no longer applies since both servers are always present; confirm the new
  steering does not include this word, or update to whatever the new framing says)

#### 4g. Update `agents_section_mcp_llm_routes_explore_not_individual_tools`
- Adjust assertions to match new dual-surface framing

#### 4h. Add `mcp_llm_forward_migration_converges_single_to_two_registrations`
(see §3 above)

#### 4i. Update `reconcile_harness_preserves_host_content` (step 2)
The `.mcp.json` fixture seeds a `grove` entry and an `other` entry. Update
to also seed a `grove-explore` entry, and assert that transitioning to Skill
strips both `grove` and `grove-explore` while preserving `other`.

---

### 5. Test Changes — `cli/tests/cli.rs` integration tests

`reconcile_harness_transition_matrix` (S03-T04 gate test) already covers all
5×5 ordered pairs via the updated `assert_*` helpers. No separate integration
test needed for the matrix.

Integration tests to update in `cli/tests/cli.rs`:

#### 5a. `mcp_llm_steering_block_idempotency`
After both runs, assert `.mcp.json` contains BOTH `grove` and `grove-explore`
keys with correct args.

#### 5b. `mcp_llm_agents_md_created_and_appended`
No assertion change needed for mcp.json (test is AGENTS.md-focused), but
any existing assertion on `grove` args needs updating.

#### 5c. `mcp_llm_dry_run_output_shape`
Assert dry-run output also mentions `grove-explore` registration.

---

### 6. Test Changes — `core/src/doctor.rs`

`harness_matrix_clean_fixtures_all_ok` will fail because
`seed_harness_for_mode(McpLlm)` seeds `["serve", "--explore"]` which no longer
matches `expected_mcp_args(McpLlm) = Some(&["serve"])`.

#### 6a. `seed_harness_for_mode(Mode::McpLlm)`
```rust
Mode::McpLlm => {
    write_mcp_json(dir, &["serve"]);             // structural server
    write_explore_mcp_json(dir);                 // explore server entry
    write_claude_md(dir, "mcp__grove-explore__explore"); // new marker
    write_agents_md(dir);
}
```
Add helper `fn write_explore_mcp_json(dir: &Path)` that writes
`{"mcpServers": {"grove-explore": {"command": "grove-explore", "args": []}}}`.

Note: `harness_matrix_clean_fixtures_all_ok` passes with both-servers seeded
because `expected_explore_args(McpLlm)` documents the expected shape; no new
doctor check is added (out of scope for T04 — ACs don't require it).

#### 6b. Update drift-scenario fixtures for McpLlm
In `mcp_llm_mode_without_agents_md_is_warn` and `explore_config_absent_is_fail`:
change `["serve", "--explore"]` → `["serve"]` in `write_mcp_json` calls, and
`"mcp__grove__explore"` → `"mcp__grove-explore__explore"` in `write_claude_md` calls.

---

## Files to Modify

| File | Change summary |
|------|---------------|
| `core/src/harness.rs` | Add `EXPLORE_SERVER_KEY`, fix `expected_mcp_args(McpLlm)`, add `expected_explore_args`, update `expected_claude_marker(McpLlm)`, update unit tests |
| `cli/src/init.rs` | Rename `write_mcp_json_explore` → `write_json_mcp_explore_server` (repurpose body), add explore-registration helpers, extend `reconcile_harness_for`, update all three branches of `assert_mcp_json_consistent`, update steering content, update/rename/add unit tests |
| `cli/tests/cli.rs` | Update mcp-llm integration tests to assert both-servers layout |
| `core/src/doctor.rs` | Update `seed_harness_for_mode(McpLlm)` and drift-test fixtures |

No new crates, no new cargo dependencies, no schema changes.

---

## Data Model Changes

None. `Mode::McpLlm` remains a legal mode value; no new modes, no config
schema change, no `grove.lock` format change.

---

## Testing Strategy

- All existing tests that pass before T04 must still pass after.
- The `reconcile_harness_transition_matrix` test covers all 20 ordered A→B
  pairs. The updated `assert_mcp_json_consistent` now requires BOTH `grove` and
  `grove-explore` entries for McpLlm, **and** requires `grove-explore` absent for
  all other modes — closing the strip-half gap (formerly AC4/AC2 hole).
- Forward migration test confirms pre-split layouts converge on first run.
- Host-content preservation test confirms neither the `grove` nor
  `grove-explore` strip removes host-authored `.mcp.json` entries.
- `harness_matrix_clean_fixtures_all_ok` in `doctor.rs` confirms the doctor
  sees clean state after `init --as mcp-llm` with the updated fixtures.

Command: `cargo test --release --locked`
Lint: `cargo clippy --all-targets --workspace --locked -- -D warnings`

---

## Acceptance Criteria Mapping

| AC | Addressed by |
|----|-------------|
| AC1 — both servers registered + dual steering | §2i (reconcile_harness_for), §2k/2l (steering) |
| AC2 — single reconcile_harness writer | §2i (single `reconcile_harness_for` function owns all writes + strip) |
| AC3 — forward migration | §3, §4h |
| AC4 (gate test b) — full transition matrix | §4a updates all three branches of `assert_mcp_json_consistent` including grove-explore strip; matrix test now validates strip half for all non-McpLlm transitions |
| AC5 — `Mode::LEGAL` unchanged | No mode added/removed |
| AC6 — host content preserved | §4i |
| AC7 — workspace green | §2c removes `write_mcp_json_explore` by repurposing it; zero dead-code lints. Full test + clippy run. |

---

## Operational Impact

- **Version bump:** material — `mcp-llm` mode behaviour changes (new
  registration written).
- **Regeneration:** existing `mcp-llm` users run `grove init --as mcp-llm`
  once; the forward migration path converges their `.mcp.json` automatically.
- **Security scan:** not required.
