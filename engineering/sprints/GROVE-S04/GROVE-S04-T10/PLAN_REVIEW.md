# PLAN REVIEW — GROVE-S04-T10: Gate test (a) — full sidebench re-run on the split binary (user-supervised rig)

(standalone review)

## Verdict: **Approved**

The plan is thorough, technically accurate, and honestly scopes a task whose
actual acceptance bar (the 347-case re-run) is user-supervised per D1. Every
load-bearing factual claim was independently verified against the real code
and the sibling repos — all check out. Advisory notes below; none block
implementation.

---

## Independent Verification (claims checked against actual code/repos)

1. **`grove serve --explore` is a hard error post-T03** — ✅ VERIFIED.
   `cli/src/main.rs:405-409`: `Cmd::Serve { explore: true, .. }` prints
   "`--explore and --standard have been removed from grove serve. Use
   `grove-explore serve` …`" and exits non-zero. The `--explore`/`--standard`
   flags are `#[arg(hide = true)]` replacement-error stubs. The harness
   cannot run against the old surface.

2. **`grove-explore serve` is the default subcommand; `serve <repo>` works** —
   ✅ VERIFIED. `grove-explore/src/main.rs`: `Cli { cmd: Option<Cmd> }` with
   `main()` doing `cli.cmd.unwrap_or(Cmd::Serve { path: "." })`. `Cmd::Serve`
   takes a positional `path` with `default_value = "."`. So both
   `grove-explore serve <repo>` and bare `grove-explore <repo>` serve the
   explore MCP surface. No `--explore` flag exists on this binary.

3. **The grove-explore binary exposes the `explore` MCP tool** — ✅ VERIFIED.
   `explore_tool_spec()` returns `"name": "explore"`. Combined with the MCP
   client config's server key `"grove"`, the exposed tool name is
   `mcp__grove__explore` — matching the harness's
   `--allowedTools "mcp__grove__explore"` filter. The tool surface is
   preserved across the split.

4. **Harness config shape quoted in the plan is exact** — ✅ VERIFIED.
   `is-grep-enough/studies/fastcontext-sidebench/run-grove-explore.sh:45`:
   `{ "mcpServers": { "grove": { "command": "$GROVE", "args": ["serve",
   "--explore", "$clone"] } } }`. The per-cell config write (lines 31-38)
   writes `c["explore"]["steering"/"base_url"/"model"/"provider"]` into
   `$clone/.grove/config.json`.

5. **`.grove/config.json[explore]` is opaque passthrough and the new binary
   reads it the same way the old path did** — ✅ VERIFIED, and this is the
   plan's most important claim. `core/src/config.rs:93`:
   `pub explore: Option<serde_json::Value>` (opaque, per stack-checklist's
   "Opaque config sections" rule). `grove-explore/src/main.rs#serve_main`
   loads via `GroveConfig::load(&root)` then
   `serde_json::from_value::<ExploreConfig>(grove_cfg.explore.unwrap())` —
   **byte-for-byte the same config path** the pre-T03 `grove serve --explore`
   used (`determine_surface` → `GroveConfig::load` → `grove_cfg.explore`).
   The harness's config writes to `.grove/config.json[explore]` are still
   read correctly by the split binary. The `ExploreConfig::load` method
   (which reads a standalone `.grove/explore.json`) is **not** used by
   `serve_main`; the plan's "no config-file move" claim holds.

6. **Prompts are byte-identical** — ✅ VERIFIED. `diff` of
   `grove/explore/src/prompts/explore_v2.system.md` (1818 B) against
   `grove-explore-model/prompts/explore-v2.system.txt` (1818 B) →
   BYTE-IDENTICAL. ADR 0004's byte-identical reference-harness claim is
   confirmed at the prompt layer.

7. **The 80.6 / 347 reference is real and the serve params are as quoted** —
   ✅ VERIFIED. `grove-explore-model/experiments/registry.jsonl` entry
   `base-q4-v2-hf`: `holdout_mean: 80.6, holdout_n: 347, quant: Q4_K_M,
   prompt: explore-v2.system.txt, harness_fixes: on`. Notes string carries
   `llama-server -c 98304 -np 4 --cache-type-k/v q8_0 -ngl 99`. The plan's
   handoff command matches the registry.

8. **The two-harness research finding is accurate** — ✅ VERIFIED.
   `run-grove-explore.sh` launches the real compiled MCP server and drives
   Claude Code over `mcp__grove__explore` (6 hardcoded cells, its own
   ctx/grounding/completeness metrics). `grove-explore-model/scripts/
   run_eval.py` is the literal 347-case/80.6 source but talks to the served
   model directly and shells to `grove <verb> --json` — it never invokes the
   explore MCP tool. The plan's characterization of both is correct, and
   its decision to **surface the scope-interpretation question to the user
   rather than silently resolve it** is exactly right.

## Spec Compliance

The task is a D1 "prepare + hand off" task: the user runs/supervises the
local rig; this task removes every blocker so the run is a single supervised
command. The plan's scope (build binaries, prepare the harness-pointing fix,
document the pinned reference command, scaffold the comparison write-up)
maps cleanly onto all five task ACs and respects the D1 role boundary. The
plan does not attempt to execute the rig unattended, and explicitly records
the handoff + ask-user checkpoint — consistent with AC5.

The plan's own AC checklist adds a sixth item (the scope question must be
surfaced, not silently resolved) that is **not in the task prompt** but is a
sound guard given the research finding. This is a plan strengthening, not a
scope creep.

## Advisory Notes (implementer should heed, non-blocking)

1. **Preserve the MCP server key as `"grove"`.** The harness fix is a
   binary+args swap, but the implementer must NOT also rename the
   `mcpServers` key from `"grove"` to `"grove-explore"`. The tool name
   Claude Code exposes is `mcp__<serverKey>__<toolName>`; renaming the key
   would yield `mcp__grove-explore__explore`, which the harness's
   `--allowedTools "mcp__grove__explore"` filter would silently drop (the
   outer agent would see no explore tool and either hang or answer from
   prior knowledge — a false-pass risk). The plan's "binary + args swap
   only" phrasing implies this but does not state it as a guard. State it.

2. **The harness path variable needs repointing.** `run-grove-explore.sh:6`
   sets `GROVE=…/target/release/grove`. The fix needs a second variable
   (e.g. `GROVE_EXPLORE=…/target/release/grove-explore`) or a repoint,
   since the `command` field now targets a different binary. Minor, but
   the "Files to Modify" table describes only the MCP config block — the
   variable declaration is also touched.

3. **The recommended scope path may require more than a binary+args swap.**
   If the user/architect confirms the plan's recommendation (use
   `grove-explore-model`'s 347-question set as the pinned reference, but
   drive it through `fastcontext-sidebench` as the execution path), the
   implementer should note that `run-grove-explore.sh` currently runs **6
   hardcoded cells**, not a parameterized 347-question harness. Plumbing
   the 347 questions through fastcontext-sidebench is non-trivial harness
   work not enumerated in the "Files to Modify" table. The plan flags the
   scope question for confirmation; the implementer should treat the
   table as describing the *minimum* (binary+args swap) and be prepared
   for additional harness work if the recommended path is adopted. This is
   not a plan defect — it is an honest consequence of the deferred
   decision.

4. **Regression guard is well-conceived.** The plan's testing strategy
   includes confirming `grove serve --explore .` still fails fast with the
   replacement error — a good guard against the harness fix masking a real
   bug (silent re-enabling of the removed surface). Retain this check.

## Conclusion

The plan is accurate, honest about its open scope question, and respects the
D1 user-supervised boundary. All technical claims verified against the actual
code and sibling repos. Approved; the advisory notes above should be carried
into implementation as guards, not as blockers.