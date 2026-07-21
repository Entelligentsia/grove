# PLAN — GROVE-S04-T10: Gate test (a) — full sidebench re-run on the split binary (user-supervised rig)

🗻 *grove Architect*

**Task:** GROVE-S04-T10
**Sprint:** GROVE-S04
**Estimate:** M

---

## Objective

Prepare everything the user needs to execute ADR 0004's gate test (a) —
prove the `grove-explore` split introduced **no inner-loop behavior change**
— and record the outcome. Concretely: produce a current release build of the
split `grove`/`grove-explore` binaries, fix the now-broken harness pointing
in the `is-grep-enough` sidebench so it targets the split binary instead of
the deleted `grove serve --explore` surface, hand the user the exact
reference-run command (llama.cpp serving `base-q4-v2-hf`, pinned sampling),
and write up the comparison against the 80.6 reference once the
user-supervised rig produces a score. This task does not run the local LLM
rig itself (D1: user runs/supervises it) — it removes every blocker so that
run is a single supervised command.

## Approach

**1. Binaries (this repo, mechanical).** Rebuild release binaries at
`target/release/grove` and `target/release/grove-explore` from current
`main` (post-T01/T02/T03/T05/T09) with `cargo build --release --locked`, and
confirm both exist and start cleanly (`grove --version`, `grove-explore
--version`). This is the "split binary" artifact the gate test compares
against the pre-split reference.

**2. Reconcile the reference-harness identity (research finding — record,
don't silently resolve).** Two candidate "sidebench" harnesses exist across
the workspace and this plan traced both before recommending scope:

   - `is-grep-enough/studies/fastcontext-sidebench/run-grove-explore.sh` —
     launches the *actual compiled MCP server* (today: `grove serve
     --explore <repo>`) and drives Claude Code over the real
     `mcp__grove__explore` tool. This is the **only** harness in the
     workspace that exercises the real, compiled `grove-explore-core` agent
     loop end-to-end — i.e. the only one that can actually detect an
     inner-loop regression from the split. Its own metrics (ctx/wall/
     grounding/completeness over 6 cells) are a different, narrower slice
     than the 347-case reference and do **not** themselves reproduce "80.6".
   - `grove-explore-model/scripts/run_eval.py` — the source of the literal
     "347-case holdout, mean 80.6" figure (`experiments/registry.jsonl`,
     entry `base-q4-v2-hf`; `runs/base-q4-v2-hf/summary.md`). It drives a
     served model directly via an OpenAI-compatible client and shells
     straight to `grove <verb> --json` for the 7 structural tools — it
     **never invokes the `explore`/`grove-explore` MCP tool at all**, so a
     green re-run of it is expected almost by construction (T01 already
     proved the structural CLI verbs byte-identical) and would not, by
     itself, validate the thing that actually moved in this sprint (the
     Rust agent loop now living in the `grove-explore` binary).
   - Cross-check performed: `explore/src/prompts/explore_v2.system.md`
     (this repo) is byte-identical to `grove-explore-model/prompts/
     explore-v2.system.txt` — confirming ADR 0004's "reference harness ships
     byte-identical" claim is about this shared prompt/harness-fix content,
     which both harnesses trace back to.

   **Recommendation carried into implementation:** treat
   `grove-explore-model`'s 347-question set and `base-q4-v2-hf` GGUF/prompt/
   sampling configuration as the *pinned reference combination* (per AC2 —
   what to serve, what questions, what settings), but treat the
   `is-grep-enough/studies/fastcontext-sidebench` harness as the *execution
   path* that must actually drive those questions through the compiled
   `grove-explore` binary (per AC1's literal repo location, and per ADR
   0004's own framing of gate test (a) as validating the binary, not a
   Python-side re-implementation). This is a **material scope
   interpretation**, not a mechanical fact, and must be confirmed with the
   user before the run is treated as authoritative — flagged again below and
   raised explicitly at plan review.

**3. Harness-pointing fix in `is-grep-enough` (prepared here, committed
there — per this task's Context: "bench changes commit in is-grep-enough,
not here").** `run-grove-explore.sh`'s per-cell MCP config currently
launches:
```
{ "mcpServers": { "grove": { "command": "$GROVE", "args": ["serve", "--explore", "$clone"] } } }
```
Post-T03 this is a hard error — `grove serve --explore` now exits with a
"removed, use grove-explore serve" message (verified: `cli/src/main.rs`
hides `--explore`/`--standard` behind a replacement-error dispatch). The
fix is a binary + args swap only: point at the new `grove-explore` binary
(`target/release/grove-explore`) with the plain `serve <repo>` invocation
(explore is `grove-explore`'s default subcommand; no `--explore` flag
exists or is needed). The `.grove/config.json` `explore` section rewrite
the script already does (steering/base_url/model/provider) is unaffected —
T01 kept that section an opaque `serde_json::Value`, no config-file move
happens until Stage 3. Prepare this change as a patch/diff artifact this
task can hand to the user for review and commit in `is-grep-enough`; do not
commit it from this repo.

**4. Hand off the supervised run.** Document the exact commands the user
runs locally: start `llama-server` serving `base-q4-v2-hf` (per the pinned
`registry.jsonl` entry: Q4_K_M quant, `-c 98304 -np 4 --cache-type-k/v
q8_0 -ngl 99`), then execute the (fixed) sidebench cells against the split
`grove-explore` binary. Since the user runs/supervises the rig (D1), this
task cannot execute that step in an unattended pipeline run — record the
handoff explicitly in PROGRESS.md, including an explicit ask-user
checkpoint if a human is present in this session, or a documented
follow-up if not.

**5. Comparison + write-up.** Once a score is available, compare it to the
80.6 reference within the study's accepted run-to-run variance. Any delta
gets attributed to known variance or flagged as a blocking split
regression (per AC3 — the sprint does not close over an unexplained
regression). Record the run per the study's evidence conventions
(`evidence/`/`reports/` in `is-grep-enough`, and/or a new
`experiments/registry.jsonl` entry in `grove-explore-model` if that's
confirmed as where this reference is tracked) and link it from this task's
PROGRESS.md.

## Files to Modify

No source files in this repo (`grove`) require modification — T01/T02/T03/
T05/T09 already shipped the split binaries and their approved behavior.
This task's changes are:

| File | Repo | Change | Rationale |
|---|---|---|---|
| `target/release/grove`, `target/release/grove-explore` | grove (this repo) | Fresh release rebuild, no source change | Produce the current split binaries as the gate-test artifact |
| `run-grove-explore.sh` (MCP launch config block) | is-grep-enough (prepared here, committed there) | Swap `grove serve --explore` for `grove-explore serve` | `grove serve --explore` is now a hard error post-T03; the bench cannot run against the split without this |
| `PROGRESS.md` | this task's artifact dir | New — comparison table, evidence links, handoff record | Required deliverable per Artifacts Involved |

## Data Model Changes

None. No `.forge/store/` schema, config schema, or `grove` on-disk config
format changes. `.grove/config.json`'s `explore` section format is
untouched (confirmed opaque-passthrough per T01).

## Testing Strategy

- `cargo build --release --locked` — confirms both binaries build clean
  from current `main`.
- `cargo test --release --locked` — full workspace suite green before
  treating the binaries as gate-test artifacts (baseline health check; not
  a substitute for the sidebench itself).
- `./target/release/grove --version` / `./target/release/grove-explore
  --version` — binaries start and identify correctly.
- `./target/release/grove serve --explore .` — confirm this still fails
  fast with the documented replacement-error message (regression guard: if
  this ever silently started serving again, the harness-pointing fix would
  be masking a real bug rather than working around an intentional removal).
- Manual/user-supervised: the fixed sidebench cells run against
  `grove-explore` on the reference rig; score compared to 80.6 within
  accepted variance. This is the actual gate test and cannot be automated
  in this pipeline run (D1).

## Acceptance Criteria

- [ ] `target/release/grove` and `target/release/grove-explore` are current
      release builds off `main` (post T01/T02/T03/T05/T09).
- [ ] The `is-grep-enough` sidebench's `grove serve --explore` invocation is
      identified and a corrected `grove-explore serve` pointing change is
      prepared (patch/diff, or applied+documented if the user grants
      cross-repo write access in-session); not committed from this repo.
- [ ] The reference-harness scope question (fastcontext-sidebench vs.
      grove-explore-model as the source of the 347-case/80.6 methodology) is
      explicitly surfaced to the user/architect before the run is treated as
      authoritative — not silently resolved by assumption.
- [ ] The pinned reference combination (llama.cpp, `base-q4-v2-hf`, same
      347-case holdout, same sampling settings) is documented as the exact
      command handoff for the user-supervised rig.
- [ ] PROGRESS.md records: binaries built + verified, harness-pointing fix
      prepared, the handoff commands, and — once available — the comparison
      table against 80.6 with an explicit pass/fail/variance verdict.
- [ ] Any unexplained regression is called out as blocking per AC3; it is
      not waved through.

## Operational Impact

- **Version bump:** none — no `grove` source, tool-spec, or schema change.
- **Regeneration:** none.
- **Security scan:** not required.
- **Cross-repo coordination:** the only lasting artifact outside this repo
  is the `is-grep-enough` harness-pointing fix, which commits in that repo,
  not here, per this task's Context note.
- **Backwards compatibility:** N/A — no shipped interface changes.
