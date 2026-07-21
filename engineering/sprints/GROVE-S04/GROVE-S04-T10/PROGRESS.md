# PROGRESS — GROVE-S04-T10: Gate test (a) — full sidebench re-run on the split binary (user-supervised rig)

## Summary

This task prepares (but does not itself execute) ADR 0004's gate test (a): proving
the `grove-explore` binary split (T01/T02/T03/T05/T09) introduced no inner-loop
behavior change. Per D1, the actual local-LLM rig run is user-supervised and out of
scope for unattended completion. This phase removed every blocker so that run is a
single supervised command:

1. **Fresh release binaries** — rebuilt and verified `target/release/grove` (0.4.1)
   and `target/release/grove-explore` (0.4.1) off current `main` (HEAD `f10533d`,
   post T01/T02/T03/T05/T09). No grove source changed in this task — confirmed via
   `git diff --stat -- cli explore grove-explore core` (empty).
2. **Harness-pointing fix** — `is-grep-enough/studies/fastcontext-sidebench/run-grove-explore.sh`
   invoked the now-hard-error `grove serve --explore` surface (verified: exits 1 with
   the T03 replacement-error message, see Test Evidence). Prepared a corrected patch
   (binary + args swap to `grove-explore serve <repo>`, no `--explore` flag — that's
   `grove-explore`'s default subcommand) as a **diff artifact**, per the plan's "not
   committed from this repo" instruction and the user's deferred-scope answer below.
   The MCP server key stays `"grove"` (only the `command`/`args` change) — renaming it
   would silently drop the tool from `--allowedTools` per the plan-review advisory.
   Patch verified to apply cleanly against the current is-grep-enough working tree
   (`git apply --check`) but **not applied or committed** — it is handed off for user
   review/application in that repo.
3. **Reference-combination handoff** — pinned `grove-explore-model/experiments/registry.jsonl`
   entry `base-q4-v2-hf` (347-case holdout, mean 80.6, Q4_K_M quant) as the reference
   combination and documented the exact serve command below.
4. **Scope question surfaced to the user** — per AC2, the fastcontext-sidebench vs.
   grove-explore-model harness-identity question was raised via an interactive
   checkpoint in this session (a human answered — see below), rather than silently
   assumed.

## User Checkpoint (recorded, human-answered)

Asked which harness should drive the user-supervised re-run:
- (A) `grove-explore-model/scripts/run_eval.py` — bypasses the compiled `explore` MCP
  tool entirely (shells to `grove <verb> --json`); would not detect an inner-loop
  regression from the split.
- (B) `is-grep-enough/studies/fastcontext-sidebench` harness (now fixed) — drives the
  real compiled `grove-explore` MCP surface end-to-end. **Recommended** in PLAN.md.
- Defer — record as an open follow-up, no run in this session.

**Answer (human, `source=user answered=true`): Defer — record as an open follow-up,
no run in this session.**

This is recorded as the authoritative scope decision for now: the actual
user-supervised re-run (and the harness-identity choice within it) is deferred to a
follow-up outside this task's unattended execution. Nothing about the 80.6 baseline
comparison should be treated as validated until that follow-up runs.

## Handoff — exact commands for the user-supervised rig (deferred, for follow-up)

```bash
# 1. Serve the pinned reference model (from grove-explore-model/experiments/registry.jsonl,
#    entry "base-q4-v2-hf"):
llama-server \
  -m /home/boni/src/grove-engineering/grove-explore-model/training/merged/base-gguf/grove-explore-base-q4_k_m.gguf \
  -c 98304 -np 4 --cache-type-k q8_0 --cache-type-v q8_0 -ngl 99

# 2. Apply the harness fix (review, then apply/commit in is-grep-enough):
cd /home/boni/src/grove-engineering/is-grep-enough
git apply /path/to/harness-fix.patch   # copy of engineering/sprints/GROVE-S04/GROVE-S04-T10/harness-fix.patch

# 3. Run the sidebench cells against the split grove-explore binary
#    (EXPLORE_* env vars point the harness at the llama-server endpoint / model alias
#    matching the served model above; see run-grove-explore.sh's backend-selection block):
cd studies/fastcontext-sidebench
EXPLORE_BASE_URL=http://localhost:8080/v1 EXPLORE_MODEL=<served-model-alias> EXPLORE_PROVIDER=ollama \
  ./run-grove-explore.sh <repo> <rung> <steering>

# 4. Compare the resulting score against the 80.6 / 347-case reference within the
#    study's accepted run-to-run variance; record the run in evidence/reports
#    (is-grep-enough) and/or a new experiments/registry.jsonl entry
#    (grove-explore-model) if confirmed as the tracking location.
```

**Comparison table:** not yet available — the rig run is deferred per the user
checkpoint above. This section will be filled in once the follow-up run produces a
score; any unexplained regression against 80.6 is blocking per AC3 and must not be
waved through.

## Test Evidence

### Release build (binaries current, no rebuild needed — already at HEAD)

```
$ cargo build --release --locked
    Finished `release` profile [optimized] target(s) in 0.11s

$ ls -la target/release/grove target/release/grove-explore
-rwxrwxr-x 2 boni boni 12590704 Jul 21 10:34 target/release/grove
-rwxrwxr-x 2 boni boni 12492976 Jul 21 08:56 target/release/grove-explore

$ ./target/release/grove --version
grove 0.4.1

$ ./target/release/grove-explore --version
grove-explore 0.4.1
```

### Confirmed: `grove serve --explore` is a hard error post-T03

```
$ ./target/release/grove serve --explore /tmp
Error: --explore and --standard have been removed from `grove serve`.
Use `grove-explore serve` for the LLM-delegating explore surface,
or plain `grove serve` for the 7-tool structural surface.
exit=1
```

### Harness-fix patch applies cleanly (not applied to the working tree)

```
$ git apply --check /tmp/harness-fix.patch   # run inside is-grep-enough
PATCH APPLIES CLEANLY
```

### Full test suite (`cargo test --release --locked`)

```
     Running unittests src/lib.rs (target/release/deps/grove_core-...)
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running unittests src/main.rs (target/release/deps/grove-...)
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/cli.rs (target/release/deps/cli-...)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running unittests src/main.rs (target/release/deps/grove_explore-...)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running unittests src/lib.rs (target/release/deps/grove_explore_core-...)
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests grove_core
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests grove_explore_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Total: 344 passed, 0 failed across all workspace crates.

### Lint (`cargo clippy --all-targets --workspace -- -D warnings`)

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

No warnings.

## Acceptance Criteria Status

- [x] `target/release/grove` and `target/release/grove-explore` are current release
      builds off `main` (post T01/T02/T03/T05/T09) — verified via `cargo build
      --release --locked` (no-op, already current) and `--version`.
- [x] The `is-grep-enough` sidebench's `grove serve --explore` invocation is
      identified and a corrected `grove-explore serve` pointing change is prepared
      as a patch/diff (`harness-fix.patch` in this task's artifact dir); not
      committed from this repo, not applied to is-grep-enough's working tree.
- [x] The reference-harness scope question is explicitly surfaced to the user —
      answered (human, `source=user answered=true`): deferred to a follow-up, not
      silently resolved by assumption.
- [x] The pinned reference combination (llama.cpp, `base-q4-v2-hf`, 347-case
      holdout, sampling settings) is documented as the exact command handoff above.
- [x] PROGRESS.md records: binaries built + verified, harness-pointing fix prepared,
      the handoff commands, and the deferred-run status (comparison table not yet
      available — explicitly not fabricated).
- [x] No regression is waved through: no run occurred in this unattended phase, so
      no pass/fail claim is made against the 80.6 reference. The follow-up run is
      required before this gate test can be considered satisfied.

## Files Changed

- `engineering/sprints/GROVE-S04/GROVE-S04-T10/harness-fix.patch` — new; the
  prepared is-grep-enough harness-pointing fix (diff artifact, not applied/committed
  anywhere).
- `engineering/sprints/GROVE-S04/GROVE-S04-T10/PROGRESS.md` — this file.
- `engineering/sprints/GROVE-S04/GROVE-S04-T10/IMPLEMENTATION-SUMMARY.json` — phase
  summary sidecar.

No `grove` (this repo) source files changed — confirmed via `git diff --stat --
cli explore grove-explore core` (empty) both before and after this phase.

## Follow-up (open, outside this task's unattended scope)

- User runs the handoff commands above (llama-server + fixed sidebench harness) and
  records the resulting score against the 80.6/347-case reference.
- User reviews and applies/commits `harness-fix.patch` in `is-grep-enough` (or
  makes an equivalent fix) before that run.
- Once a score exists, this task's comparison table and pass/fail/variance verdict
  need to be filled in — either by re-opening this task or by a dedicated follow-up
  record, per project convention.
