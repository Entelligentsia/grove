//! System-prompt + convergence-message assembly for the inner explorer.
//!
//! This is the `base-q4-v2-hf` reference combination (the interim winner in
//! `grove-explore-model/experiments/registry.jsonl`, scored 80.6 on the 347-case
//! holdout, served on llama.cpp): a **single flat system prompt**
//! (`explore-v2.system.txt`, embedded verbatim as [`prompts/explore_v2.system.md`])
//! whose output contract is **bare location lines** (`lang:path#symbol@line` /
//! `path:line`), not a `<final_answer>` block. There are no merit/plan-first/strict
//! arms and no `submit_plan` recon phase — the plan-first machinery is gone.
//!
//! The forced-answer and nudge messages are the reference harness's **H1/H2**
//! model-visible corrections (`grove-explore-model/scripts/run_eval.py`,
//! `prompts/explore-prompt-v2.md`): the forced prompt no longer offers an
//! empty-answer exit (H1), and both the nudge and forced prompts carry a concrete
//! example line instead of "as your instructions specify" (H2). All strings are
//! copied byte-for-byte from the validated harness so the port cannot drift.

use std::path::Path;

use super::config::Steering;

/// The single flat exploration prompt (bare-location-line output contract),
/// embedded verbatim from the reference `explore-v2.system.txt`.
const EXPLORE_V2_SYSTEM: &str = include_str!("prompts/explore_v2.system.md");

/// The system prompt for the inner explorer.
///
/// `steering` and `root` are accepted for call-site compatibility but no longer
/// select prompt variants: the reference winner is a single flat prompt with no
/// arm-specific steering and no `${WORK_DIR}` template vars (the tools scope to
/// `root` and return repo-relative paths, so the model needs no injected cwd).
pub fn system_prompt(_steering: Steering, _root: &Path) -> String {
    EXPLORE_V2_SYSTEM.trim_end().to_string()
}

/// Soft wrap-up nudge, injected a couple of calls before the turn cap so the
/// model can stop voluntarily. Carries a concrete example (H2), verbatim from
/// `run_eval.py` (`harness_fixes` branch).
pub const NUDGE: &str = "You are almost out of tool calls. If you already have the code points you need, stop searching now and give your final answer: one location per line, like `python:django/db/models/query.py#QuerySet@326`, and nothing else.";

/// The forced-final-answer message injected when the loop stops without an
/// answer. H1 (no empty-answer exit) + H2 (concrete example), verbatim from
/// `run_eval.py`.
pub const FORCE_FINAL_ANSWER: &str = "Stop searching now — the exploration phase is over. Based ONLY on the tool outputs above, give your final answer: the code points you located, one location per line, like `python:django/db/models/query.py#QuerySet@326`, and nothing else. An empty answer scores zero — give your best-effort locations from the tool outputs above.";

/// The stricter re-prompt used on a leaked forced turn (retry-on-leak), verbatim
/// from `run_eval.py`.
pub const FORCE_STRICT: &str = "You have NO tools available — do NOT emit any tool call or any `<...>` tag text. Output ONLY the code points you already found, one location per line, like `python:django/db/models/query.py#QuerySet@326`. Give your best effort from the tool outputs above — do not return nothing.";

/// The re-prompt used when a malformed (leaked) tool call arrives mid-loop, so
/// the model re-issues it as a proper function call instead of terminating.
/// Verbatim from `run_eval.py`'s in-loop `leak_retry` branch.
pub const LEAK_RETRY: &str = "Your previous message contained a tool call in an invalid format, so it did NOT run and produced no result. Re-issue that tool call now as a proper function/tool call (not text). Then continue.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_is_the_flat_v2_contract() {
        let p = system_prompt(Steering::Standard, &std::env::temp_dir());
        // Bare-location-line contract, no <final_answer> wrapper, no arm steering.
        assert!(p.contains("location lines"), "v2 output contract present");
        assert!(!p.contains("<final_answer>"), "no legacy final_answer wrapper");
        assert!(!p.contains("[GROVE_SECTION]"), "no arm placeholder");
        assert!(!p.contains("MANDATORY TOOL POLICY"), "no strict arm");
        assert!(!p.contains("${WORK_DIR}"), "no template vars");
        // All steering values collapse to the same prompt.
        assert_eq!(p, system_prompt(Steering::Strict, &std::env::temp_dir()));
        assert_eq!(p, system_prompt(Steering::Balanced, &std::env::temp_dir()));
    }

    #[test]
    fn convergence_messages_carry_a_concrete_example() {
        let example = "python:django/db/models/query.py#QuerySet@326";
        assert!(NUDGE.contains(example));
        assert!(FORCE_FINAL_ANSWER.contains(example));
        assert!(FORCE_STRICT.contains(example));
        // H1: the forced prompt must not invite an empty answer.
        assert!(!FORCE_FINAL_ANSWER.contains("return an empty answer"));
    }
}
