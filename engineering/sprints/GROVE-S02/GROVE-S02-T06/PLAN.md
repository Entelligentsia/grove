# PLAN — GROVE-S02-T06
## `grove init --as mcp-llm` — target, harness steering (CLAUDE.md/AGENTS.md), .mcp.json

---

## Objective

Add `Target::McpLlm` to `grove init --as`, wiring the full mcp-llm flow: grammar provisioning, a first-run TUI to produce `.grove/explore.json`, an explore-mode `.mcp.json`, and idempotent sentinel steering blocks in both `CLAUDE.md` and `AGENTS.md`.

---

## Context & Dependencies

- **T04** established `grove serve --explore` (the flag) and `Surface::Explore` (the health-gated runtime surface). `.mcp.json` for mcp-llm must register `["serve", "--explore"]`, not just `["serve"]`.
- **T05** delivered `config_tui::run(root, maybe_cfg)` — the full-screen ratatui TUI that creates/updates `.grove/explore.json`. It already hard-fails on non-TTY with "requires an interactive terminal". Init delegates the TUI launch to this function unchanged.
- Existing targets (`Mcp`, `Skill`, `Both`, `Grammars`) must be byte-identical. No changes to their code paths.

---

## Approach

### Step 1 — `Target::McpLlm` variant (`cli/src/init.rs`)

Add a new variant to the `Target` enum (after `Both`, before `Grammars`):

```rust
/// Register grove in explore-mode (`.mcp.json` with `serve --explore`) + sentinel
/// steering blocks in `CLAUDE.md` and `AGENTS.md` directing the agent through
/// `mcp__grove__explore`. Runs the config TUI on first init (when
/// `.grove/explore.json` is absent); skips TUI on re-runs and `--dry-run`.
McpLlm,
```

The existing `writes_mcp()` and `writes_steering()` helpers are **not** modified — McpLlm is handled as a dedicated early-return branch in `write_harness`, so those helpers only need to remain accurate for existing variants.

### Step 2 — Non-TTY guard in `run()` (`cli/src/init.rs`)

Before calling `provision_project`, insert:

```
if target == McpLlm
   && !dry_run
   && .grove/explore.json does not exist
   && stdout is not a terminal
→  bail!("requires an interactive terminal …")
```

The guard is **conditional on explore.json absence**: re-runs (where explore.json already exists) do not require a TTY because the TUI is skipped. First runs require a TTY to collect the explore backend configuration. This makes integration-test idempotency tests feasible (pre-seed explore.json, run binary twice, non-TTY OK on second run).

### Step 3 — McpLlm flow in `run()` (`cli/src/init.rs`)

After `provision_project` returns:

1. **Dry-run path** (`provisioned.is_empty()` after dry-run short-circuit):
   For McpLlm, print the planned harness actions (`.mcp.json`, `CLAUDE.md`, `AGENTS.md`) and return. Existing targets: unchanged behavior.

2. **TUI launch** (before `write_harness`):
   If `target == McpLlm` and `.grove/explore.json` does not yet exist, call `config_tui::run(root, None)` to create it. If explore.json already exists (re-run), skip the TUI.

3. **Harness write**: call `write_harness(root, target)` as before. McpLlm handled in its own branch (see Step 4).

4. **Output**: after writing, print a McpLlm-specific "ready" block describing `mcp__grove__explore` and the 7-tool structural fallback.

### Step 4 — `write_harness` McpLlm branch (`cli/src/init.rs`)

Add an early-return branch at the top of `write_harness`:

```rust
if target == Target::McpLlm {
    let langs = registry::locked_langs(&root.join("grove.lock"))?;
    wrote.push(write_mcp_json_explore(root)?);
    wrote.push(write_claude_md(root, &langs, target)?);
    wrote.push(write_agents_md(root, &langs, target)?);
    return Ok(wrote);
}
```

Everything below that branch is the existing, unmodified code for `Mcp/Skill/Both/Grammars`.

### Step 5 — `write_mcp_json_explore` (`cli/src/init.rs`)

New function, modelled on `write_mcp_json` but registers `["serve", "--explore"]`:

```rust
fn write_mcp_json_explore(root: &Path) -> Result<String> {
    // same read-existing / merge JSON / write pattern as write_mcp_json
    // diff: args: ["serve", "--explore"]  instead of  args: ["serve"]
}
```

Preserves other MCP servers already registered. Returns `".mcp.json (explore-mode registration)"`.

### Step 6 — `claude_section` McpLlm arm (`cli/src/init.rs`)

Add a third branch in `claude_section` (before the existing `if target.writes_mcp()` check). The McpLlm steering block:

- Directs the agent to route **all** codebase exploration through `mcp__grove__explore` (not the 7 structural tools individually).
- States that while the local model is healthy, the 7 structural tools are **not exposed** — `mcp__grove__explore` is the sole surface.
- States that if the provider goes down, grove automatically falls back to the 7 structural tools (these will then appear in `tools/list` per T04's fallback contract).
- Uses the same `CLAUDE_START` / `CLAUDE_END` sentinels so idempotency is automatic (no new constants needed).

The existing two arms (`writes_mcp()` → MCP tools block; else → skill/CLI block) are not touched.

### Step 7 — `write_agents_md` + `agents_section` (`cli/src/init.rs`)

New functions following the exact same sentinel/idempotency pattern as `write_claude_md`:

```rust
fn write_agents_md(root: &Path, langs: &[String], target: Target) -> Result<String> {
    // Same read/replace-between-sentinels/append/create logic as write_claude_md
    // File: root.join("AGENTS.md")
    // Section: agents_section(langs, target)
    // Returns: "AGENTS.md (explore-mode steering)"
}

fn agents_section(langs: &[String], target: Target) -> String {
    // For McpLlm: harness-agnostic framing (AGENTS.md is read by non-Claude harnesses)
    // Same sentinel markers (CLAUDE_START / CLAUDE_END) for grove's block identity
    // Directs: use the grove MCP server's `explore` tool for all code questions
    // Fallback: if provider unavailable, the 7 structural tools are surfaced
}
```

`AGENTS.md` content (McpLlm only): directs the agent to use grove's `explore` tool (via whatever MCP tool prefix the harness assigns) rather than built-in search/read tools. Uses harness-neutral language ("the `explore` tool exposed by the grove MCP server") rather than the Claude-specific `mcp__grove__explore` prefix.

### Step 8 — `README.md` + `docs/setup.md`

- `README.md`: add `mcp-llm` to the `--as` reference where the existing targets are listed.
- `docs/setup.md`: add a new section documenting `grove init --as mcp-llm`, what it writes (`.mcp.json` in explore mode, `CLAUDE.md`, `AGENTS.md`), and the first-run/re-run TUI behaviour.

### Step 9 — Tests

**`cli/src/init.rs` unit tests** (direct function calls, no binary):

| Test | What it verifies |
|---|---|
| `claude_section_mcp_llm_routes_explore_not_individual_tools` | `claude_section` with McpLlm: names `mcp__grove__explore`; does not name `mcp__grove__outline`; contains fallback mention |
| `agents_section_mcp_llm_routes_explore_not_individual_tools` | `agents_section` with McpLlm: names `explore` tool; does not use Claude-specific prefix; mentions fallback |
| `write_harness_mcp_llm_writes_mcp_json_explore_and_steering` | `write_harness` with McpLlm: `.mcp.json` contains `--explore`; `CLAUDE.md` exists; `AGENTS.md` exists; 3 entries returned |
| `write_agents_md_creates_then_updates_idempotently` | AGENTS.md created on first call; second call replaces block in place — exactly one sentinel pair |
| `write_agents_md_appends_below_existing_content` | Existing hand-written AGENTS.md is preserved; grove block appended |
| `write_mcp_json_explore_registers_with_explore_flag` | `.mcp.json` has `"args": ["serve", "--explore"]` |
| `write_mcp_json_explore_preserves_other_servers` | Other servers in existing `.mcp.json` kept intact |

**`cli/tests/cli.rs` integration tests** (binary invocation):

| Test | Setup | What it verifies |
|---|---|---|
| `mcp_llm_dry_run_output_shape` | Seed a `.rs` file; run `grove init --as mcp-llm --dry-run` (non-TTY, no explore.json) | Prints "detected", "mcp.json", "CLAUDE.md", "AGENTS.md"; writes no files; exits 0 |
| `mcp_llm_steering_block_idempotency` | Pre-seed explore.json + `.rs` file; run `grove init --as mcp-llm` twice | Both runs exit 0; `CLAUDE.md` has exactly 1 grove sentinel block; `AGENTS.md` has exactly 1 grove sentinel block |
| `mcp_llm_agents_md_created_and_appended` | (a) no AGENTS.md: run init → AGENTS.md created; (b) hand-written AGENTS.md: run init → hand-written content preserved, grove block appended once | AC3 + AC6 |

For both integration tests that write files: pre-seed `.grove/explore.json` with a minimal valid JSON so the non-TTY guard is bypassed and the TUI is skipped.

---

## Files to Modify

| File | Change |
|---|---|
| `cli/src/init.rs` | Add `Target::McpLlm`; update `run()`, `write_harness()`; add `write_mcp_json_explore()`, `write_agents_md()`, `agents_section()`; update `claude_section()`; add unit tests |
| `cli/tests/cli.rs` | Add 3 new integration test cases |
| `README.md` | Add `mcp-llm` to `--as` target list |
| `docs/setup.md` | Add `--as mcp-llm` section |

---

## Data Model Changes

None. `grove.lock` and `.grove/explore.json` formats are unchanged. `.mcp.json` schema is unchanged (the new target writes `"args": ["serve", "--explore"]` vs `["serve"]`).

---

## Testing Strategy

1. Existing tests **must pass unmodified** (AC4). Run `cargo test --release --locked` after the change and confirm all prior init cases pass.
2. Unit tests in `init.rs` exercise every new function in isolation (no binary, no TTY, no real grammars — seeded `grove.lock`).
3. Integration tests in `cli/tests/cli.rs` exercise end-to-end binary behaviour, using the same fake cache + dead registry URL pattern from the existing `init_provisions_and_wires_harness_per_target` test.
4. Clippy must be clean: `cargo clippy -- -D warnings`.
5. No `fastcontext` string anywhere in new code (AC7).

---

## Acceptance Criteria Mapping

| AC | Covered by |
|---|---|
| 1. `Target::McpLlm` in value enum, `--help`, README | Steps 1, 8 |
| 2. Flow: provision → TUI → harness | Steps 2, 3 |
| 3. `.mcp.json` with `serve --explore`; CLAUDE.md + AGENTS.md idempotent | Steps 5, 6, 7 |
| 4. Existing targets unchanged | Dedicated early-return in `write_harness`; existing paths untouched |
| 5. Dry-run prints planned actions; non-TTY fails fast | Steps 2, 3 |
| 6. Integration tests: dry-run shape, idempotency, AGENTS.md create/append | Step 9 |
| 7. Warning-clean, clippy-clean, tests green, no "fastcontext" | Step 9 + testing strategy |

---

## Operational Impact

- **Material change**: No — adds a new `--as` mode without altering the existing three; no version bump required (per task spec).
- **Backward compatibility**: Existing `--as mcp|skill|both|grammars` code paths are structurally isolated behind their own branches; adding `McpLlm` cannot affect them.
- **New files written in users' projects**: `.mcp.json` (explore-mode), `CLAUDE.md` (explore sentinel), `AGENTS.md` (new file or appended). Re-running is idempotent.
- **First-run TTY requirement**: Users running `grove init --as mcp-llm` for the first time need an interactive terminal for the TUI. CI re-runs (after explore.json created) work without TTY.
