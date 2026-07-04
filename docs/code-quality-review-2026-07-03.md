# Grove — Architecture, Implementation & Testing Review

**Date:** 2026-07-03
**Scope:** the `grove/` repo (~9,300 lines of Rust across `core` and `cli` crates), graded against the target state: clean, well-tested, modular, maintainable code. Security intentionally excluded.
**Method:** three independent deep reviews (architecture, implementation quality, testing) plus a process/hygiene pass. Clippy and the full test suite were run from a clean build; coverage was measured with `cargo llvm-cov`.

## Grade summary

| Dimension | Grade |
|---|---|
| Architecture & Modularity | **A-** |
| Implementation Quality | **A-** |
| Testing Practices | **A-** |
| Documentation & Process | **B+** |
| **Overall** | **A-** |

The headline: this codebase actually does what its documentation claims, which is rarer than it should be. The layering rule ("main/mcp only format, ops is the engine") was verified arm-by-arm and holds with zero violations. Clippy is warning-clean from a cold build. 209 tests pass in ~14 seconds with real seams and regression anchors. What keeps every dimension out of the A range is the same root cause: fast recent iteration (the core/cli crate split, the experimental explore subsystem, `grove tap`) has outrun the docs, the coverage claim, and a few corners of the code.

---

## 1. Architecture & Modularity — A-

**The stated architecture is enforced, not aspirational.** Every `Cmd::*` arm in `main()` and every MCP tool arm is `ops::X(...)` followed by formatting (`cli/src/main.rs:177-347`, `cli/src/mcp.rs:435-503`) — no business logic was found in either face. `ops` returns typed `Symbol`/`Defect`/`CallSite`/`FileMap` structs and never prints. The dependency graph is a clean DAG (`main`/`mcp` → `ops` → `engine` + `registry`) with no back-edges.

### Strengths

- **Principled crate split.** `core` (published as `grove-cst`) is clap-free and ratatui-free; `cli` holds parsing, formatting, TUI, and harness glue. Even `init` is split correctly: grammar provisioning in `core/src/init.rs`, file-writing glue in `cli/src/init.rs`. `core/src/lib.rs:50-65` curates a deliberate re-export surface.
- **The language extension point is data, not code.** `Profile` deserializes from `manifest.json`; adding a language is a registry directory drop with no Rust changes — verified, exactly as claimed.
- **The experimental explore/ subsystem is excellently fenced.** Zero references from any stable core module; it could be deleted touching only additive glue (lib.rs re-exports, `Surface::Explore` arms, CLI verbs). At runtime, `determine_surface` (`cli/src/mcp.rs:44-69`) degrades to the standard surface on any explore failure — the stable surface survives explore breakage.
- **Adding a new tool is cheap:** `map` took one ops function, one clap arm, one MCP spec + dispatch arm. Three thin touchpoints, no framework ceremony.

### Weaknesses

- **The explore toolset shells out to grove's own binary** (`core/src/explore/toolset.rs:405-478`) instead of calling `ops` in-process — losing the in-process grammar cache and paying a subprocess spawn + reparse per tool call. Documented as a deliberate "faithful port of the proven bench agent," so acceptable while experimental, but this must be revisited before mcp-llm stabilizes.
- **CLAUDE.md's module map is stale:** it lists `cli/src/ops.rs`, `cli/src/engine.rs`, `cli/src/registry.rs`, `cli/src/fetch.rs`, `cli/src/ingest.rs` — all five now live in `core/src/` after the workspace split.
- Small debris: `ignore = "0.4"` is an unused dependency in `cli/Cargo.toml`; `CallSite.source` is a stringly `"structural"`/`"textual"` field where a two-variant enum belongs; the source-id XOR argument validation is duplicated between the two faces; `ops::project` returns `serde_json::Value` from the engine layer (arguable, since both faces share it).
- No output-format abstraction — a third format (`--format tsv`) would mean editing ~13 match arms. Tolerable at two formats; worth knowing.

## 2. Implementation Quality — A-

**Unusually clean Rust for a young project.** A fresh-from-clean `cargo clippy --all-targets` produces **zero warnings** on both crates. Of 267 grep hits for `unwrap()`/`expect(`, only **9 are in non-test code**, and every one is sound (mutex-lock poison propagation, or guarded by a preceding check, e.g. `cli/src/init.rs:207-209`). Error handling is uniformly `anyhow` + `.with_context(...)`. Zero TODO/FIXME/HACK debt in source.

### Strengths

- **Comment quality is the standout.** Comments explain constraints and cite issue numbers rather than restating code: the `.d.ts` exclusion rationale (`core/src/ops.rs:30-43`), structural-vs-textual callers design (`ops.rs:304-316`), refusing silent `as u8` truncation (`cli/src/mcp.rs:529-540`). All five `#[allow(dead_code)]`s carry justifications.
- **Deliberate performance awareness:** parse-tree reuse in `callers`, compact JSON specifically to save agent tokens, shared traversal helpers (`for_each_source`, `kind_matches`) that keep `symbols`/`callers`/`map` from re-implementing walking.

### Weaknesses

- **A few oversized functions.** Worst is `run_explore_reporting` (`core/src/explore/agent.rs:148-295`, ~148 lines) — a turn loop with a four-arm dispatch chain nested three deep; the recon/execute phases deserve extraction. Others (`main()`'s flat dispatch match, `tool_specs`' 98 lines of JSON literals) are long but trivially flat.
- **Real copy-paste in `cli/src/init.rs`:** `write_mcp_json` vs `write_mcp_json_explore` (~20 identical lines) and `write_claude_md` vs `write_agents_md` (verbatim sentinel-splice logic — the doc comment even admits it). Two small helpers would remove ~40 duplicated lines. This is the only genuine logic duplication found.
- Minor: `messages.clone()` per model turn in the agent loop is O(turns²) — harmless at the ≤7-turn cap, worth an `Arc` if the cap grows. `cli/src/tap.rs`'s hand-rolled HTTP proxy is the least robust code in the tree, acceptably flagged as a debug aid.

## 3. Testing Practices — A-

**Measured, not claimed:** 209 tests, **0 failures**, ~14s wall. Coverage measured at **78.5% lines / 81% functions** — below the ~83% the docs claim (the drift comes from newer code: `tap.rs` and the explore toolset).

### Strengths

- **Tests are behavioral and regression-anchored.** Line-numbering pinned to `grep -n` convention across all stub grammars (issue #31); the `detail: 256` u8-wrap bug has a named test; a schema test guards that no MCP tool emits top-level `anyOf` (which clients silently drop); a parse-counter test pins the double-parse regression (#13); a multibyte-boundary truncation panic has a dedicated regression test.
- **Real seams.** `ChatClient` is a trait with a scripted `FakeClient`, a `DownClient`, and a progress `Recorder` — the entire bounded agent loop is tested with zero network. The MCP server is tested without stdio by calling `handle()`/`call_tool()` directly, including protocol-error paths (`-32601`, `-32602`).
- **The integration suite is hermetic and low-brittleness:** pins `GROVE_REGISTRY` to the dev stub, throwaway temp fixtures, structural assertions (JSON parse + key presence) rather than golden bytes. Covers all 7 verbs plus init/lock/index and the explore-fallback path.

### Weaknesses

- **`explore/toolset.rs` is the worst-covered live logic (52.5% lines)** — its tests check composition and input guards but never execute the inner Read/Glob/Grep tool bodies against a fixture tree. This is active code in the mcp-llm path, not a network boundary.
- **The TUI is only one-third tested:** the Elm-style reducer (`update.rs`) has 12 solid tests at 84%, but `view.rs` is 0% and the event loop 9% — no ratatui `TestBackend` tests.
- **Some unit tests can go vacuous:** several engine tests silently `return` when a grammar isn't resolvable (`engine.rs:658,733,836`) — on a misconfigured machine they pass while testing nothing (the integration suite does cover those paths unconditionally).
- Network paths (`fetch` 62%, `ingest` 49%) are untested beyond the error-before-fetch boundary — a documented, conscious tradeoff, though there's no mocked test of hash-verification failure handling.

## 4. Documentation & Process — B+

The documentation *volume and quality* are well above typical: a 431-line VISION, dated CHANGELOG, RELEASING runbook, a proper ADR (scope-aware resolution), a full docs site, and CLAUDE.md files that make the repo genuinely navigable for both humans and agents. What drags the grade down is **drift and gates**:

- **Stale claims:** the coverage figure (~83% vs measured 78.5%) and the module map (five paths pointing at the wrong crate) mean the docs currently mislead in two places. Docs this good create trust; drift in trusted docs is worse than no docs.
- **CI is thinner than the project's own standards.** The repo convention is "warning-clean builds," yet CI runs only `cargo build` + `cargo test` on a single Linux runner — no `cargo clippy -- -D warnings`, no `fmt --check`, no coverage tracking, and no macOS/Windows test job despite releasing binaries for 5 platforms.
- **Repo hygiene:** `fix_og.py` (a one-off script) and `server.pid` (a runtime artifact) are committed at the repo root; `server.pid` should be deleted and gitignored.

---

## Prioritized recommendations

1. **Add clippy + fmt gates to CI** (`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`) and at least a macOS test job. The code already passes — this is a one-file change that locks in the current standard. Highest leverage, lowest cost.
2. **Fix the two doc drifts:** update CLAUDE.md's module map to the `core/src/` paths and the coverage figure to ~78%.
3. **Test the explore toolset's tool bodies** (Read/Glob/Grep against a temp fixture tree) — it's the largest pool of untested live logic and it sits in the subsystem under active iteration.
4. **Extract the two `cli/src/init.rs` helpers** to kill the only real copy-paste (~40 lines).
5. **Before stabilizing mcp-llm:** replace the toolset's shell-out-to-own-binary with in-process `ops` calls, and break up `run_explore_reporting`.
6. **Housekeeping:** remove `fix_og.py` and `server.pid` from version control; drop the unused `ignore` dependency; consider an enum for `CallSite.source`.

**Bottom line:** the stable surface (CLI + 7-tool MCP server + registry) is in genuinely excellent shape — disciplined, tested, and documented. Nearly every deduction across all four dimensions traces to the experimental explore work moving faster than its tests and docs, which is exactly where you'd want your debt concentrated. Items 1–2 are an afternoon of work and would move the overall grade to a straight A.
