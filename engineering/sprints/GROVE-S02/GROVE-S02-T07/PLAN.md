# Plan — GROVE-S02-T07: Integration tests, naming guard, docs

## Objective

Harden the mcp-llm sprint end-to-end: close cross-task test gaps that no single
prior task owns, enforce the "no fastcontext" naming rule mechanically, document
the mcp-llm mode for users and contributors, and bump the version to mark the
sprint done.

---

## Approach

The task is five distinct concerns addressed in order:

1. **Cross-cutting tests** — new integration tests and one unit test that fill
   gaps not owned by T04 or T06.
2. **Naming guard** — a deterministic test that will fail on any future
   re-introduction of "fastcontext" in source or docs.
3. **Docs** — prose updates to `README.md`, `CHANGELOG.md`, and `CLAUDE.md`.
4. **Version bump** — bump `0.2.0 → 0.3.0` across the four version-of-truth files
   and add a dated CHANGELOG section per RELEASING.md.
5. **Full gates** — `cargo build --release --locked --workspace`, `cargo clippy`,
   `cargo test --release --locked` must be green before commit.

No behaviour changes to production code — all changes are either new test code or
documentation/CHANGELOG. This means no module-level Rust code changes beyond
adding tests.

---

## Files to Modify

| File | Change |
|---|---|
| `cli/tests/cli.rs` | Add 4 new integration tests (AC1a, AC1b, AC2) |
| `core/src/explore/agent.rs` | Add 1 unit test in `#[cfg(test)]` (AC1c) |
| `README.md` | Add mcp-llm mode section (AC3) |
| `CHANGELOG.md` | Add dated `[0.3.0]` section (AC3 + version bump) |
| `CLAUDE.md` | Add `grove config`, `grove serve --explore/--standard` to Commands; add `core::explore` to architecture map (AC3) |
| `cli/Cargo.toml` | Bump version `0.2.0 → 0.3.0` and the `grove-cst` pin |
| `core/Cargo.toml` | Bump version `0.2.0 → 0.3.0` |
| `dist/npm/package.json` | Bump `"version"` `0.2.0 → 0.3.0` |
| `Cargo.lock` | Refreshed by `cargo update -p grove-cst -p grove-cst-cli` |

---

## Data Model Changes

None. No schema changes to `.forge/store/` or `.forge/config.json`.

---

## Testing Strategy

### AC1a — `grove config` non-TTY fast-fail

Add test `config_in_non_tty_fails_fast` to `cli/tests/cli.rs`.

Strategy: spawn `grove config` with `Stdio::piped()` stdout (so it is not a TTY);
use a deadline loop (5-second timeout) to guard against a hang. Assert:
- exit code non-zero
- stderr contains the string `"interactive terminal"` (from the non-TTY guard in
  `config_tui::run()`)

Implementation note: `Command::output()` blocks forever if the process hangs, so
we must use `spawn()` + `try_wait()` polling loop + `kill()` on timeout, then
`wait_with_output()` to collect stderr.

### AC1b — Init idempotency: dry-run twice + `.mcp.json` deduplication

Two tests:

**`mcp_llm_dry_run_twice_is_stable`**: Run `grove init --as mcp-llm --dry-run`
twice in the same project directory (no explore.json). Verify:
- Both runs succeed (exit 0)
- No files are written (`.mcp.json`, `CLAUDE.md`, `AGENTS.md`, `grove.lock` all absent)
- `--dry-run` output is stable (same key strings in both stdout responses)

**`mcp_llm_mcp_json_no_duplicate_grove_entry`**: Pre-seed `.grove/explore.json`,
run `grove init --as mcp-llm` twice. Parse the resulting `.mcp.json` and assert:
- Exactly 1 key `"grove"` under `mcpServers` (object key semantics mean the
  second write overwrites — deduplication is implicit in `write_mcp_json_explore`,
  which uses `doc["mcpServers"]["grove"] = json!({…})`)
- The `args` field is `["serve", "--explore"]`

The existing `mcp_llm_steering_block_idempotency` already covers CLAUDE.md and
AGENTS.md sentinel blocks, so those assertions are not duplicated here.

### AC1c — Allowlist enforcement (unit test via `ScriptedClient`)

Add test `allowlist_enforcement_find_refused` to the `#[cfg(test)]` block inside
`core/src/explore/agent.rs`.

Strategy: build an `ExploreConfig` with `allowed_tools: vec!["grep".to_string()]`
(omits `find`). Script a `ScriptedClient` with two turns:
1. Model calls `find` with some args. Since `find` is NOT in `build_full_toolset(&["grep"])`,
   `is_in_toolset("find", &active_tools)` returns `false` → `corrective_refusal` is
   returned (not `dispatch_tool`).
2. After seeing the corrective tool-result, model gives a text answer.

Assertions:
- `run_explore()` succeeds
- Final answer text matches turn-2 response
- `answer.turns == 1` (loop ran 1 full turn before the text answer)

Note: `dispatch_tool` is not even reached for non-toolset tools; the refusal is
produced by `corrective_refusal()` in the agent loop. This tests the complete
enforcement path.

### AC2 — Naming guard

Add test `naming_guard_no_fastcontext_in_source` to `cli/tests/cli.rs`.

Strategy: use `std::fs::read_to_string` and `walkdir`-style manual tree walk
(using `ignore::Walk` is available via the `ignore` crate which is already a
dependency) OR use `std::fs::read_dir` recursively. Since this is a test (not
production code), simple `std::process::Command::new("grep")` or a direct Rust
recursive walk is acceptable.

Files and directories to scan (per AC2 and D5):
- `core/src/` — recursive `.rs` files
- `cli/src/` — recursive `.rs` files
- `README.md` — top-level file
- `skills/` — recursive all files

Assert: none of the scanned file contents contains `"fastcontext"` (case-insensitive).
The test uses `CARGO_MANIFEST_DIR` to anchor the path to the workspace root, not
a runtime directory.

Implementation note: the test must use a relative-to-manifest-dir path (the test
binary's cwd may differ from the project root). Use
`Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()` to reach the workspace
root from `cli/tests/cli.rs`.

### AC5 — Full gates green

Run after all changes:
```bash
cargo build --release --locked --workspace
cargo clippy -- -D warnings
cargo test --release --locked
```

---

## Docs Plan (AC3)

### `README.md` — new "Delegated local-LLM mode" section

Insert a new section after `## The tools`, titled
`## Delegated local-LLM mode (mcp__grove__explore)`.

Content:
- **What it is**: one MCP tool (`mcp__grove__explore`) that the outer agent calls
  with a natural-language question; grove's inner Rust explorer agent drives the
  tool-calling loop locally using the configured LLM.
- **Setup**: `grove init --as mcp-llm` → launches the setup TUI; `grove config` to
  change settings later.
- **Three modes** (one-line trade-off each):
  - `standard` — inner model chooses tools naturally (merit/grep-natural)
  - `balanced` — two-phase plan→execute (best grounding, highest wall)
  - `aggressive` — grove-first strong steering (cost/quality sweet spot)
- **Health semantics**: startup probe (`/models`) decides the surface:
  healthy → explore-only; unhealthy at startup → transparent fallback to the 7
  structural tools; mid-session loss → recoverable `isError` with restart hint.

### `CHANGELOG.md` — new dated section

Replace the planned `[Unreleased]` with `[0.3.0] - 2026-07-02`. Include:
- `grove init --as mcp-llm` — new init target, harness wiring, TUI first-run
- `grove config` — TUI to edit explore config at any time
- `grove serve --explore` — exclusive `mcp__grove__explore` surface
- Health-gated startup with structural fallback
- Inner explorer engine (`core::explore`) — pure-Rust bounded loop, 3 modes,
  allowlisted shell dispatch, OpenAI-compat client (Ollama + llama.cpp)

### `CLAUDE.md` — two additions

1. **Architecture map** (the `## Architecture — one engine, two faces` section):
   Update the old layout (which uses `src/*.rs`) to reflect the workspace split
   and add the `core::explore` module lines:
   ```
   core/src/explore/    inner explorer engine (mcp-llm mode)
     mod.rs             re-exports; public surface is run_explore()
     config.rs          ExploreConfig — .grove/explore.json serde + atomic save
     client.rs          ChatClient trait + OpenAiCompatClient + health_probe()
     agent.rs           bounded loop (≤ 25 turns / 128 KiB), mode state machine
     toolset.rs         tool schema registry, build_*_toolset(), dispatch_tool()
     steering.rs        system prompts for Standard / Balanced / Aggressive
   cli/src/config_tui/  full-screen ratatui TUI (grove config verb)
   ```

2. **Commands block** (the `## Commands` section):
   Add `grove config` and `grove serve --explore/--standard` to the setup/init
   commands list:
   ```
   grove init [path] [--as mcp|skill|both|mcp-llm] [--dry-run]
   grove config [path]           # open the explore config TUI (requires TTY)
   grove serve [path] [--explore] [--standard]  # MCP server; mode flags override config
   ```

---

## Version Bump (AC Operational Impact)

Following RELEASING.md, bump `0.2.0 → 0.3.0`:

| File | Change |
|---|---|
| `cli/Cargo.toml` | `version = "0.3.0"` and `grove-cst = { …, version = "=0.3.0" }` |
| `core/Cargo.toml` | `version = "0.3.0"` |
| `dist/npm/package.json` | `"version": "0.3.0"` |
| `Cargo.lock` | `cargo update -p grove-cst -p grove-cst-cli` |
| `CHANGELOG.md` | dated `## [0.3.0] - 2026-07-02` section |

---

## Acceptance Criteria Mapping

| AC | Test / Change |
|---|---|
| AC1a — `grove config` non-TTY | `config_in_non_tty_fails_fast` in `cli/tests/cli.rs` |
| AC1b — dry-run twice stable | `mcp_llm_dry_run_twice_is_stable` in `cli/tests/cli.rs` |
| AC1b — `.mcp.json` dedup | `mcp_llm_mcp_json_no_duplicate_grove_entry` in `cli/tests/cli.rs` |
| AC1c — allowlist enforcement | `allowlist_enforcement_find_refused` in `core/src/explore/agent.rs` |
| AC2 — naming guard | `naming_guard_no_fastcontext_in_source` in `cli/tests/cli.rs` |
| AC3 — README mcp-llm section | `README.md` new section |
| AC3 — CHANGELOG | `CHANGELOG.md` `[0.3.0]` section |
| AC3 — CLAUDE.md | `CLAUDE.md` architecture + commands update |
| AC4 — stack-checklist pass | Documented in PR description (MCP tool schema plainness, stdio hygiene, exit codes, `--json` coverage, no new unsafe, rustls-only) |
| AC5 — full gates green | `cargo build`, `cargo clippy`, `cargo test` all green |

---

## Operational Impact

- **Version bump**: `0.2.0 → 0.3.0` — prepared in this task per RELEASING.md.
- **Security scan**: no new shell dispatch paths introduced; all shell tool dispatch
  goes through the existing allowlist guard in `toolset::dispatch_tool()`.
- **No regeneration required** (no grammar, registry, or MCP schema changes).
- All tests are additive — no existing tests are modified or removed.
