# PLAN_REVIEW — GROVE-S02-T07: Integration tests, naming guard, docs (standalone review)

**Verdict:** Approved

## Independent verification performed

I read the actual source, not the plan's claims. Every load-bearing symbol and
assumption the plan relies on was confirmed on disk:

| Plan assumption | Verified against | Result |
|---|---|---|
| `config_tui::run()` non-TTY guard emits `"interactive terminal"` | `cli/src/config_tui/mod.rs:39` | ✓ exact string present |
| `.mcp.json` dedup is implicit via object key | `write_mcp_json_explore` (`cli/src/init.rs:221`) uses `doc["mcpServers"][MCP_SERVER_KEY] = json!(…)`, `args: ["serve","--explore"]` | ✓ correct — key overwrite dedups; args match AC |
| CLAUDE.md/AGENTS.md idempotency already covered | `mcp_llm_steering_block_idempotency` (`cli/tests/cli.rs:588`) | ✓ exists — not duplicating is correct |
| `ScriptedClient`, `corrective_refusal`, `is_in_toolset`, `build_full_toolset`, `run_explore` all exist | `core/src/explore/agent.rs`, `toolset.rs` | ✓ all present |
| `ExploreAnswer.turns` field for the `turns == 1` assertion | `core/src/explore/agent.rs:40` | ✓ field exists |
| AC1c is not redundant with the shell-allowlist test | `shell_binary_not_in_allowlist_is_refused` tests `dispatch_tool` shell path; AC1c tests the loop-level `is_in_toolset`→`corrective_refusal` gating | ✓ distinct code path |

## Category assessment

- **Correctness / completeness** — All five ACs are mapped to concrete tests or
  doc edits (see the plan's AC-mapping table). The five test additions
  (`config_in_non_tty_fails_fast`, `mcp_llm_dry_run_twice_is_stable`,
  `mcp_llm_mcp_json_no_duplicate_grove_entry`, `allowlist_enforcement_find_refused`,
  `naming_guard_no_fastcontext_in_source`) each target a real, verified code path.
- **Security** — AC1c exercises the allowlist-enforcement path through the real
  agent loop; the plan correctly notes `dispatch_tool` is never reached for a
  non-toolset tool. No new shell-dispatch surface introduced. rustls-only /
  no-new-unsafe recorded for the AC4 checklist pass.
- **Architecture / conventions** — Test placement matches existing conventions
  (unit test inside `agent.rs #[cfg(test)]`; integration tests in `cli/tests/cli.rs`
  reusing the `mcp_llm_setup` helper). Version bump touches the correct four files
  plus `Cargo.lock` per RELEASING.md.
- **Test design robustness** — The non-TTY test correctly avoids `Command::output()`
  (which would hang) in favour of `spawn()` + polling + `kill()` with a 5s deadline.
  The naming guard anchors to `CARGO_MANIFEST_DIR/..` (test cwd ≠ repo root) and its
  scan scope (core/src, cli/src, README.md, skills/) deliberately excludes
  `cli/tests/` so the test's own `"fastcontext"` literal cannot self-trip.

## Advisory notes (non-blocking)

1. **`answer.turns == 1` — confirm turn-counting semantics.** The field exists,
   but the exact count after a refusal→answer sequence is an off-by-one risk.
   Assert against the observed value during implementation rather than assuming.
2. **AC1c mild overlap with `hallucinated_tool_returns_corrective_refusal`.** Both
   drive `corrective_refusal` via the loop. The new test's distinct value is the
   *allowlist-config* dimension (`allowed_tools` omitting `find`). Keep that angle
   explicit in the test name/comment so its purpose is unambiguous.
3. **Naming guard walker dependency.** The plan hedges between `ignore::Walk` and a
   std recursive walk. Prefer a plain std recursive walk (or the already-present
   `walkdir`/`ignore` only if confirmed a dev-dependency) to avoid an unverified
   crate assumption — the plan's own fallback is fine.
4. **AC4 is a review-record obligation, not code.** Ensure the stack-checklist pass
   (schema plainness, stdio hygiene, exit codes, `--json`, no unsafe, rustls-only)
   is actually recorded in the PR/commit description, since no test enforces it.
