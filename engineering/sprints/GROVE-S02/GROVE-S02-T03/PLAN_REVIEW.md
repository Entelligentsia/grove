# PLAN REVIEW — GROVE-S02-T03 (standalone review)

**Inner explorer agent loop with mode steering + tool gating (`core::explore`)**

**Verdict:** Approved

The plan is feasible, complete against all 8 acceptance criteria, and correctly
grounded in the actual T01 (`config.rs`) and T02 (`client.rs`) surfaces. I verified
each dependency by reading the source rather than trusting the plan's claims.

## Independent Verification Performed

| Plan assumption | Verified against | Result |
|---|---|---|
| `run_explore(question, root, cfg, client)` signature | AC-1 + `ChatClient` trait in `client.rs` | ✅ Exact match; trait object `&dyn ChatClient` is the real seam |
| `Message::system/user/tool`, `ChatRequest::new().with_tools()`, `ChatResponse::first_message()`, `.tool_calls`, `Tool::function` | `client.rs` | ✅ All exist with the shapes the loop uses |
| `ClientError::Connection` → `ExploreError::ProviderDown` (AC-6) | `ClientError` enum in `client.rs` | ✅ `Connection { url, detail }` present; `ProviderDown { url, detail }` maps cleanly |
| `cfg.mode` (Standard/Balanced/Aggressive) + `cfg.allowed_tools` | `config.rs` `Mode`, `ExploreConfig` | ✅ Both present |
| 7 structural ops callable in-crate + serializable | `ops.rs` (`outline/symbols/source/check/callers/map/definition`) return `#[derive(Serialize)]` types | ✅ Direct-call + `serde_json` string is feasible |
| No new Cargo deps | `core/Cargo.toml` (serde_json present; `std::process::Command`) | ✅ |
| `ToolCall.arguments` is a parsed `Value` (for `submit_plan` plan extraction) | `client.rs` `normalize_arguments` | ✅ Always parsed object; `args["plan"]` extraction safe |

## Advisory Notes (address during implementation; none block approval)

1. **Per-op argument parsing is under-specified — the biggest real work item.**
   The 7 ops have heterogeneous typed signatures: `outline(file, kind)`,
   `symbols(dir, kind, name, refs, name_contains)`, `source(id_or_file, name)`,
   `map(dir, …)`, `definition(dir, name)`, `definition_at(file, row, col, dir)`,
   `check(file)`, `callers(dir, name)`. `dispatch_tool` must (a) define a distinct
   JSON-schema `parameters` per tool and (b) parse `args_json` into each op's typed
   args with graceful failure (bad/missing args → corrective tool-result string,
   not a panic). The plan glosses this as one `dispatch_tool` arm; budget for it
   and ensure malformed args degrade to a corrective message like unknown-tool does.

2. **Aggressive `tool_choice` hint conflicts with the stated design.** Loop step 3
   ("For Aggressive: also set `tool_choice` hint (prefer grove ops)") is not
   expressible via OpenAI `tool_choice` (only `auto`/`none`/one forced function —
   not "prefer a subset"), and it contradicts both AC-4 ("full toolset, grove-first
   *steering prompt*") and the test `aggressive_toolset_same_as_standard`
   ("steering is prompt-only, not schema-level"). Recommend dropping the
   `tool_choice` hint: aggressive steering should be prompt-only.

3. **`BALANCED_RECON_TURNS` comment says "from cfg" but no such config field exists.**
   `ExploreConfig` (T01) has no recon-turns field, so this is a module constant
   (default 2, matching AC-4). Correct the misleading comment, or — if per-project
   tuning is intended — that would be a config change out of this task's additive
   scope. Keep it a constant.

4. **`build_full_toolset(root, allowed_shell)` takes `root` but the schemas are
   static.** Confirm `root` is actually needed (e.g. for a cwd note in descriptions);
   otherwise drop the unused param to stay clippy-clean (AC-8).

5. **Byte-bound test realism.** `byte_bound_terminates_loop` must accumulate
   ≥128 KiB (`MAX_TOOL_RESULT_BYTES`) through scripted tool results. Either lower
   the bound behind a test seam or have the scripted client emit a large payload —
   just ensure the assertion targets `truncated: true`, not an error (AC-5).

## Categories

- **Correctness:** Loop logic (dispatch → feed back → terminate on empty tool_calls
  or bounds) is sound; phase machine for Balanced (Recon → ForceSubmit → Execute)
  matches AC-4 harness-enforcement intent.
- **Security:** Shell dispatch is safe by construction — `Command::new(binary).args(vec)`,
  no `sh -c`, allowlist checked before spawn, per-call byte cap. Meets AC-3.
- **Architecture:** Three-module split (agent/toolset/steering) keeps concerns clean
  and depends on the `ChatClient` trait, not the concrete client — testable.
- **Testing:** 10-test matrix covers all AC-7 scenarios (hallucinated tool, per-mode
  toolsets, phase transitions, allowlist refusal, turn/byte bounds, provider-down).
- **Conventions:** No new deps, clap-free, no `fastcontext` string, clippy-clean
  targeted (AC-8).
