# CODE REVIEW — GROVE-S04-T10: Gate test (a) — full sidebench re-run on the split binary (user-supervised rig)

🌿 *grove Supervisor*

**Task:** GROVE-S04-T10
**Review mode:** standalone review

---

**Verdict:** Approved

---

## Review Summary

This is a gate-test **preparation/handoff** task — no `grove` source code changed
(T01/T02/T03/T05/T09 already shipped the split). The implementation correctly
executed everything within its unattended scope: rebuilt/verified the release
binaries, prepared a correct harness-pointing patch for `is-grep-enough`, pinned
the reference combination, surfaced the harness-scope question to a genuine human
checkpoint (who deferred the run), and honestly recorded the deferred-run status
without fabricating any pass/fail claim against the 80.6 reference. All evidence in
PROGRESS.md was independently re-verified and is authentic. The deferred items
(actual rig run, score comparison, evidence recording) are explicitly user-supervised
per D1 and were deferred by a real human answer — not silently skipped.

## Checklist Results

| Item | Result | Notes |
|---|---|---|
| No grove source changed (mechanical rebuild only) | ✓ | `git diff --stat -- cli explore grove-explore core` empty — confirmed |
| Release binaries current off main | ✓ | `grove 0.4.1`, `grove-explore 0.4.1`; both present at `target/release/` |
| `grove serve --explore` confirmed hard error (regression guard) | ✓ | exit 1, correct replacement message pointing to `grove-explore serve` |
| `grove-explore serve` is default subcommand (no `--explore` flag) | ✓ | `--help` confirms `serve` is a top-level subcommand |
| Harness patch: binary path repointed (plan-review advisory) | ✓ | `GROVE=…/target/release/grove-explore` — addresses the run-grove-explore.sh:6 advisory |
| Harness patch: MCP server key `"grove"` preserved (plan-review advisory) | ✓ | `{ "mcpServers": { "grove": { "command": "$GROVE", "args": ["serve", "$clone"] } } }` — false-pass risk avoided |
| Harness patch: args swapped to `["serve", "$clone"]` | ✓ | No `--explore` flag (correct — it's the default subcommand) |
| Harness patch applies cleanly to is-grep-enough | ✓ | `git apply --check` passes (independently re-run) |
| Harness patch NOT applied/committed (cross-repo boundary respected) | ✓ | Patch saved as artifact only; is-grep-enough working tree untouched |
| Reference combination pinned correctly | ✓ | registry.jsonl `base-q4-v2-hf`: holdout_mean 80.6, holdout_n 347, quant Q4_K_M — matches handoff |
| Prompt byte-identity cross-check (ADR 0004 claim) | ✓ | `explore_v2.system.md` == `explore-v2.system.txt` — byte-identical |
| Scope question surfaced to human (not silently assumed) | ✓ | `source=user answered=true`: defer to follow-up — genuine human answer |
| `cargo test --release --locked` | ✓ | Independently re-run: 344 passed, 0 failed (matches PROGRESS exactly) |
| `cargo clippy --all-targets --workspace -- -D warnings` | ✓ | Independently re-run: no warnings |
| No pass/fail fabricated against 80.6 | ✓ | PROGRESS explicitly states comparison table "not yet available"; no run occurred |
| Version bump | N/A | No source/tool-spec/schema change |
| Security scan | N/A | Per task prompt |

## Issues Found

None blocking.

## If Approved

### Advisory Notes

1. **The actual gate test (task AC3: score vs 80.6) remains unperformed — by user
   decision, not oversight.** The user checkpoint (`source=user answered=true`)
   deferred the rig run to a follow-up. This is the single open item that gates
   sprint closure: the sprint's "no inner-loop behavior change" guarantee is not
   validated until that run produces a score within accepted variance of 80.6. This
   task's review approval reflects correct *preparation*, not gate-test satisfaction.

2. **Task AC1 literal wording partially satisfied.** AC1 says harness-pointing
   changes "are made in that repo" — the patch is *prepared* but not *applied or
   committed* in `is-grep-enough`. This matches the approved plan's "prepared here,
   committed there" instruction and the user's defer answer, so it is consistent with
   the agreed scope. The user must apply `harness-fix.patch` in `is-grep-enough`
   before the follow-up run can proceed.

3. **Handoff commands are complete and actionable.** The PROGRESS.md handoff block
   gives the exact `llama-server` invocation (matching the registry entry's serve
   params), the patch-application step, the sidebench run command with `EXPLORE_*`
   env vars, and the comparison/evidence-recording step. A user can execute the
   follow-up run from this handoff without further research.

4. **Both plan-review advisories were addressed in the patch.** The GROVE variable
   repoint (run-grove-explore.sh:6) and the MCP server key preservation (`"grove"`)
   are both present — the false-pass risk where `--allowedTools` would silently drop
   the tool is avoided.