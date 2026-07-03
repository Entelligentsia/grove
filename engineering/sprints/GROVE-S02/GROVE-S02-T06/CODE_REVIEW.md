# CODE_REVIEW — GROVE-S02-T06 (standalone review)

## `grove init --as mcp-llm` — target, harness steering (CLAUDE.md/AGENTS.md), .mcp.json

**Verdict:** Approved

Reviewed the working-tree diff (task is `implemented`, uncommitted) for
`cli/src/init.rs`, `cli/tests/cli.rs`, `README.md`, `docs/setup.md`. All seven
acceptance criteria are met and independently verified — I re-ran the test and
lint suites rather than trusting PROGRESS.md.

### Independent verification
- `cargo test -p grove-cst-cli --bins` → **51 passed, 0 failed** (incl. the 7
  new McpLlm unit tests: `claude_section_mcp_llm_*`, `agents_section_mcp_llm_*`,
  `write_harness_mcp_llm_*`, `write_agents_md_*` ×2, `write_mcp_json_explore_*` ×2).
- `cargo test -p grove-cst-cli --test cli mcp_llm` → **3 passed** (dry-run shape,
  steering idempotency, AGENTS.md create/append).
- `cargo clippy -p grove-cst-cli --all-targets -- -D warnings` → **clean**.
- `grep -rn fastcontext cli/ core/ README.md docs/` → **no matches** (AC7).

### AC-by-AC
1. **Target::McpLlm** — variant added with a full doc-comment that surfaces in
   `--help`; `ValueEnum` auto-derives `--as mcp-llm`; README `--as` list updated
   in both references. ✓
2. **Flow order** — `provision_project` → (first-run) `config_tui::run(root, None)`
   → `write_harness`. The dry-run harness print is correctly placed *before* the
   `provisioned.is_empty()` early-return (the plan-review advisory), so `--dry-run`
   still narrates the planned files. ✓
3. **Harness files** — `.mcp.json` registers `["serve","--explore"]` via the same
   read/merge pattern as `write_mcp_json` (preserves other servers, verified by
   `write_mcp_json_explore_preserves_other_servers`); CLAUDE.md + AGENTS.md use the
   shared `grove:start/end` sentinel for in-place idempotent updates. ✓
4. **Existing targets unchanged** — McpLlm is handled by a dedicated early-return
   in `write_harness`; `writes_mcp()` still returns false for McpLlm so the generic
   "ready" block never double-prints (explicit McpLlm arm precedes the
   `writes_mcp()` check). Mcp/Skill/Both/Grammars paths byte-untouched. ✓
5. **Dry-run / non-TTY** — dry-run prints planned writes and skips both the TUI
   and file writes; the non-TTY guard bails fast *only* on first run (explore.json
   absent, not dry-run, non-terminal stdout) with an init-worded message. ✓
6. **Integration tests** — all three required cases present and green. ✓
7. **Warning/clippy/test/no-fastcontext** — all confirmed above. ✓

### Advisory notes (non-blocking)
1. AC2(b) reads "pre-populated if the file exists"; the implementation instead
   *skips* the TUI entirely on re-runs (file present). This is the correct call —
   it keeps CI re-runs TTY-free and is consistent with the first-run-only non-TTY
   guard — and it matches the approved PLAN. No change required; flagged only so
   the AC wording and behaviour are reconciled if the AC is ever revisited.
2. `agents_section` / `claude_section` share the CLAUDE sentinel constants
   (`CLAUDE_START`/`CLAUDE_END`) for AGENTS.md too. Correct for idempotency, but
   the constant names now span two harness files — a future rename to a neutral
   `GROVE_START/END` would read cleaner. Cosmetic only.
