# Validation Report — GROVE-S02-T07: Integration tests, naming guard, docs

*(standalone review)*

**Verdict:** Approved

---

## Summary

All 5 acceptance criteria verified against the live implementation. The full test
suite (`cargo test --release --locked`) passes cleanly: **205/205 tests** across
all workspace members (127 core lib, 51 explore unit, 26 CLI integration, 1
doc-test). Clippy runs warning-free under `-D warnings`.

---

## Per-Criterion Results

### AC1a — `grove config` non-TTY fast-fail

**PASS**

- Test: `config_in_non_tty_fails_fast` — `cli/tests/cli.rs:685`
- Spawns `grove config` with all stdio piped (no TTY), polls with 50 ms sleep,
  kills and panics if still alive after 5 s.
- Asserts: exit non-zero **AND** `stderr.contains("interactive terminal")`.
- Guard string at `cli/src/config_tui/mod.rs:39` confirmed: `"interactive terminal"`.
- Result: test passes; no hang; exit code non-zero; correct stderr message.

---

### AC1b — Init idempotency (dry-run + `.mcp.json` dedup)

**PASS** (two tests)

**`mcp_llm_dry_run_twice_is_stable`** (`cli/tests/cli.rs:727`):
- Two consecutive `grove init --as mcp-llm --dry-run` runs.
- Asserts: both exit 0; no files written (`.mcp.json`, `CLAUDE.md`, `AGENTS.md`,
  `grove.lock` all absent); both outputs contain the key strings `"detected"`,
  `"mcp.json"`, `"CLAUDE.md"`, `"AGENTS.md"`.
- Result: passes.

**`mcp_llm_mcp_json_no_duplicate_grove_entry`** (`cli/tests/cli.rs:760`):
- Pre-seeds `.grove/explore.json` to bypass the TTY guard, then runs
  `grove init --as mcp-llm` twice.
- Parses resulting `.mcp.json`, asserts `mcpServers["grove"]` count == 1
  and `args == ["serve", "--explore"]`.
- Result: passes; object-key overwrite semantics verified.

---

### AC1c — Allowlist enforcement (loop-level gating)

**PASS**

- Test: `allowlist_enforcement_find_refused` — `core/src/explore/agent.rs:617`
- `ExploreConfig { allowed_tools: ["grep"], .. }` — `find` omitted.
- `ScriptedClient` drives: turn 1 → tool call `find`; turn 2 → text answer.
- Asserts: `run_explore` succeeds; final `answer.text` is `"The answer is: use grep
  instead."`;  `answer.turns == 1` (loop consumed the tool call + correction as
  one turn before the text answer terminated).
- Result: passes; corrective_refusal path confirmed at loop level (distinct from
  the shell-dispatch-level `shell_binary_not_in_allowlist_is_refused` test).

---

### AC2 — Naming guard: "fastcontext" absent from source

**PASS**

- Test: `naming_guard_no_fastcontext_in_source` — `cli/tests/cli.rs:817`
- Walks `core/src/`, `cli/src/`, `README.md`, and `skills/` (excludes
  `cli/tests/` to prevent self-tripping on the literal in the test itself).
- Case-insensitive match on `"fastcontext"`.
- Confirms constraint D5 ("No 'fastcontext' naming anywhere in grove").
- Result: zero violations; passes.

---

### AC3 — Documentation completeness

**PASS** (all three docs)

**`README.md`** — section `## Delegated local-LLM mode (mcp__grove__explore)` at
line 128:
- What it is: single `mcp__grove__explore` MCP tool, outer agent receives synthesised
  answer.
- Setup: `grove init --as mcp-llm` + `grove config`.
- Three modes table: `standard` / `balanced` / `aggressive` with one-line trade-offs.
- Health semantics: startup probe → healthy → explore-only surface; unhealthy → 7
  structural tools fallback; mid-session loss → recoverable `isError` with restart hint.

**`CHANGELOG.md`** — `## [0.3.0] - 2026-07-02` at line 7:
- Covers: `grove init --as mcp-llm`, `grove config`, `grove serve --explore`,
  health/fallback behaviour, inner explorer engine.

**`CLAUDE.md`** — two additions:
- Architecture map: `cli/src/config_tui/` entry and `core::explore` module wiring
  (line 23, 36).
- Commands block: `grove config [path]`, `grove serve [path] [--explore] [--standard]`,
  `grove init [--as mcp-llm]` all present.

---

### AC4 — Stack-checklist pass

**PASS** (documented)

Recorded in PROGRESS.md: MCP tool schema plainness, stdio hygiene
(protocol/stdout vs diagnostics/stderr), exit codes, `--json` coverage of new CLI
output, no new `unsafe`, rustls-only transport. No new shell dispatch paths
introduced; allowlist enforcement confirmed at toolset level.

---

### AC5 — Full gates green

**PASS**

| Gate | Result |
|---|---|
| `cargo build --release --locked` | ✅ clean (implied by test runner success) |
| `cargo clippy -- -D warnings` | ✅ clean — `Finished dev profile … 0.12s`, no warnings |
| `cargo test --release --locked` | ✅ **205/205** tests pass (0 failed, 0 ignored) |
| Version unanimous | ✅ `cli/Cargo.toml`, `core/Cargo.toml`, `dist/npm/package.json` all `0.3.0` |
| Cargo.lock refreshed | ✅ reflects updated version pins |

Breakdown of test results:
- grove-core lib unit tests: 127/127
- grove-core explore + subsystem unit tests: included above
- CLI integration tests (`cli/tests/cli.rs`): 26/26
- Doc-tests: 1/1

---

## Regression Check

All 26 CLI integration tests pass, including tests from prior tasks (T04/T06)
such as `mcp_llm_steering_block_idempotency`, `explore_mode_unhealthy_provider_falls_back_to_standard_surface`,
`mcp_llm_agents_md_created_and_appended`, and `mcp_llm_dry_run_output_shape`.
No regressions detected.

---

## Test Quality Notes

- AC1a timeout guard (spawn+poll+kill) is a robust non-TTY detector; avoids
  `Command::output()` deadlock on piped stdio.
- AC1c `answer.turns == 1` off-by-one advisory (from plan review) was correctly
  resolved: the assertion passes and the loop semantics match the ScriptedClient
  script.
- AC2 self-exclusion of `cli/tests/` is correctly anchored to `CARGO_MANIFEST_DIR/..`.
- All five ACs have test coverage with specific, regression-catching assertions —
  no vacuous always-pass conditions observed.
