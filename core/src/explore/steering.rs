//! Per-mode system prompt text for the inner explorer.
//!
//! All steering content is encoded as static strings returned by
//! [`system_prompt`]. No runtime state — mode behaviour is entirely prompt-
//! driven (AC-4). The Balanced two-phase approach is split across two
//! functions: [`system_prompt`] for phase 1 and
//! [`balanced_phase2_prompt`] for phase 2, which prepends the committed plan.

use super::config::Mode;

/// Phase-1 system prompt for Standard mode.
const STANDARD_PROMPT: &str = "\
You are an expert code-intelligence assistant backed by structural grove tools. \
Answer the user's question by exploring the repository with the tools available. \
When you have gathered enough information to give a complete, accurate answer, \
reply with your findings in plain text — no tool call in the final turn.";

/// Phase-1 system prompt for Aggressive mode.
///
/// Urges the model to prefer structural grove ops over text-search tools like
/// `grep`/`rg`. Steering is prompt-only (AC-4) — the tool schema is identical
/// to Standard; the model is persuaded, not coerced.
const AGGRESSIVE_PROMPT: &str = "\
You are an expert code-intelligence assistant. ALWAYS prefer the structural \
grove tools (outline, symbols, source, check, callers, map, definition) over \
text-search tools like grep or rg. Structural tools give you typed, structured \
data about the codebase and are far more reliable than pattern matching. Only \
fall back to grep/rg when a structural tool genuinely cannot answer the question. \
When you have gathered enough information, reply with your findings in plain text.";

/// Phase-1 system prompt for Balanced mode (recon phase).
///
/// Directs the model to use structural recon tools to build a mental map of
/// the codebase first, then call `submit_plan` when it has a concrete plan of
/// attack. The `submit_plan` call commits the plan and unlocks the full toolset
/// for phase 2.
const BALANCED_RECON_PROMPT: &str = "\
You are an expert code-intelligence assistant operating in Balanced mode. \
Phase 1 — RECONNAISSANCE: explore the repository structure using the available \
structural tools (map, symbols, outline, definition). When you have a clear, \
concrete plan for answering the question — including which files to read and \
which symbols to investigate — call `submit_plan` with your plan as a string. \
Do not attempt to answer the question yet; your only goal in this phase is to \
understand the landscape and commit a plan.";

/// Return the phase-1 system prompt for the given mode.
///
/// For Balanced mode this is the recon-phase prompt; phase-2 uses
/// [`balanced_phase2_prompt`] instead.
pub fn system_prompt(mode: Mode) -> &'static str {
    match mode {
        Mode::Standard => STANDARD_PROMPT,
        Mode::Aggressive => AGGRESSIVE_PROMPT,
        Mode::Balanced => BALANCED_RECON_PROMPT,
    }
}

/// Return the phase-2 system prompt for Balanced mode.
///
/// Prepends the committed plan as a standing hint so the model tracks its
/// own decision and executes it rather than re-exploring from scratch.
pub fn balanced_phase2_prompt(plan: &str) -> String {
    format!(
        "You are an expert code-intelligence assistant operating in Balanced mode.\n\
         Phase 2 — EXECUTE: you previously committed the following exploration plan:\n\
         \n\
         {plan}\n\
         \n\
         Now execute that plan step-by-step using the full toolset. When you have \
         gathered enough information to give a complete, accurate answer to the \
         original question, reply with your findings in plain text — no tool call \
         in the final turn."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_returns_non_empty_for_all_modes() {
        for mode in [Mode::Standard, Mode::Balanced, Mode::Aggressive] {
            let p = system_prompt(mode);
            assert!(!p.is_empty(), "empty prompt for {mode:?}");
        }
    }

    #[test]
    fn balanced_phase2_prompt_embeds_plan() {
        let plan = "1. call outline on main.rs\n2. read the Foo struct";
        let p = balanced_phase2_prompt(plan);
        assert!(p.contains(plan), "plan text must appear verbatim in phase-2 prompt");
    }
}
