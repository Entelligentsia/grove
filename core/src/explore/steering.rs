//! System-prompt assembly — a direct port of the reference bench's
//! `agent/utils.py::load_system_prompt` + `agent_factory.py`'s `[GROVE_SECTION]`
//! mechanism, with the arm mapping fixed by the sprint decision:
//!
//! - [`Steering::Standard`]  → **merit**: balanced steering, Grove as one option.
//! - [`Steering::Aggressive`]→ **coerce**: the MANDATORY grove-first steering.
//! - [`Steering::Balanced`]  → **plan-first**: merit steering; the two-phase recon
//!   notes are appended by [`crate::explore::agent`] at run time.
//!
//! Prompt text is embedded verbatim from `prompts/*.md` (copied byte-for-byte
//! from the vendored + original reference-bench trees) so the port cannot drift from
//! the validated wording.

use std::path::Path;

use super::config::Steering;

/// The base exploration prompt with a `[GROVE_SECTION]` marker and `${…}`
/// template vars (vendored `system.md`).
const SYSTEM_MD: &str = include_str!("prompts/system.md");
/// Balanced "one toolkit" Grove steering (vendored `grove_steering.md`) — merit.
const GROVE_STEERING_MERIT: &str = include_str!("prompts/grove_steering_merit.md");
/// "MANDATORY TOOL POLICY — use Grove, not Grep" (original repo) — coerce.
const GROVE_STEERING_COERCE: &str = include_str!("prompts/grove_steering_coerce.md");

/// The plan-first phase-1 note, ported verbatim from `mcp_server.py`.
pub const PHASE1_NOTE: &str = "\n\n## PLANNING PHASE\nYou are scoping WHERE to look before investigating. Tools now:\n- Grove — structure only: map, symbols, outline, definition.\n- submit_plan — records your focus area and unlocks the execution tools.\n\nDo 1-2 Grove calls to locate the relevant code (e.g. `symbols . --name-contains --name <term>`, then `map <dir>` or `outline <file>`). As soon as you can name the files and symbols involved, CALL submit_plan(focus_files, focus_symbols, steps). You CANNOT read bodies, Grep, or answer until you call submit_plan; after 1-2 Grove calls Grove closes and submit_plan is your only option — do not over-explore.";

/// Phase-2 unlock note (prefixes the recorded plan), verbatim from `mcp_server.py`.
pub const PHASE2_NOTE: &str = "EXECUTION PHASE — your plan is recorded (below) and all tools are unlocked: Grove (incl. source/callers), Read, Grep, Glob. Execute your plan to answer the ORIGINAL question, choosing whichever tool fits each step — Grove for named symbols, Grep for literal text, Read to confirm a range. Cite file:line and emit <final_answer> when done.";

/// Shown when the model calls a closed tool during recon (verbatim).
pub const RECON_CLOSED_NOTE: &str = "<system-reminder>Grove is CLOSED. Your only tool now is submit_plan. Call submit_plan(focus_files, focus_symbols, steps) from what you found.</system-reminder>";

/// Shown when Grove is used with a non-recon verb during recon (verbatim).
pub const RECON_VERB_NOTE: &str = "<system-reminder>Planning phase: Grove is limited to map/symbols/outline/definition. source/callers/Read/Grep/Glob unlock after you call submit_plan.</system-reminder>";

/// Observation returned after a successful `submit_plan` (verbatim).
pub const PLAN_RECORDED_NOTE: &str =
    "Plan recorded. Execution tools unlocked: Read, Grep, Glob, Grove (source/callers).";

/// The forced-final-answer user message injected at `max_turns + 1`
/// (verbatim from `agent.py`).
pub const FORCE_FINAL_ANSWER: &str =
    "Max number of turns reached. Please provide the final answer based on the information you have gathered.";

/// Build the system prompt for `mode`, rendering the template vars against
/// `root`. Port of `load_system_prompt` (`OS_KIND`/`SHELL_NAME`/`WORK_DIR`/
/// `WORK_DIR_LS`) followed by the `[GROVE_SECTION]` substitution.
pub fn system_prompt(steering: Steering, root: &Path) -> String {
    let grove_block = match steering {
        Steering::Aggressive => GROVE_STEERING_COERCE,
        // Standard (merit) and Balanced (plan-first) share the merit base; the
        // plan-first phase notes are layered on by the agent loop.
        Steering::Standard | Steering::Balanced => GROVE_STEERING_MERIT,
    };
    let with_grove = SYSTEM_MD.replace("[GROVE_SECTION]", &format!("\n{}", grove_block.trim_end()));
    render_template_vars(&with_grove, root)
}

/// Substitute the `${OS_KIND}` / `${SHELL_NAME}` / `${WORK_DIR}` /
/// `${WORK_DIR_LS}` builtins, matching `load_system_prompt`.
fn render_template_vars(template: &str, root: &Path) -> String {
    let os_kind = std::env::consts::OS; // "linux" / "macos" / "windows"
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
    let work_dir = root.display().to_string();
    let work_dir_ls = list_dir(root);
    template
        .replace("${OS_KIND}", os_kind)
        .replace("${SHELL_NAME}", &shell)
        .replace("${WORK_DIR}", &work_dir)
        .replace("${WORK_DIR_LS}", &work_dir_ls)
}

/// Newline-joined top-level entry names of `root` (port of `os.listdir`).
fn list_dir(root: &Path) -> String {
    let mut names: Vec<String> = match std::fs::read_dir(root) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => return String::new(),
    };
    names.sort();
    names.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merit_and_coerce_select_distinct_grove_blocks() {
        let root = std::env::temp_dir();
        let std_p = system_prompt(Steering::Standard, &root);
        let agg_p = system_prompt(Steering::Aggressive, &root);
        assert!(
            agg_p.contains("MANDATORY TOOL POLICY"),
            "aggressive uses coerce steering"
        );
        assert!(
            !std_p.contains("MANDATORY TOOL POLICY"),
            "standard uses merit steering"
        );
        assert!(!std_p.contains("[GROVE_SECTION]"));
        assert!(!agg_p.contains("[GROVE_SECTION]"));
    }

    #[test]
    fn template_vars_are_rendered() {
        let p = system_prompt(Steering::Standard, &std::env::temp_dir());
        assert!(!p.contains("${WORK_DIR}"), "WORK_DIR substituted");
        assert!(!p.contains("${OS_KIND}"), "OS_KIND substituted");
    }
}
