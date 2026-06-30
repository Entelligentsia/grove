---
id: generic-skills
name: Generic Meta-Skills
description: Baseline capabilities for support and orchestration roles.
role: Generic
applies_to: [orchestrator, collator, supervisor]
summary: >
  Baseline coordination, information synthesis, and basic tooling that
  every support role needs regardless of domain.
capabilities:
  - Schedule tasks and manage dependency resolution
  - Hand off context between roles cleanly
  - Aggregate progress from multiple agents
  - Perform basic file and git operations
  - Monitor logs and events for triggers
file_ref: .forge/skills/generic-skills.md
---

## Generation Instructions

When generating the project-specific skill set for support roles (e.g., Orchestrator, Collator) in `.forge/skills/generic-skills.md`, the generator must:
1. Cross-reference the `installedSkills` list in `.forge/config.json`.
2. Map the universal skills listed below to the specific implementation names found in `installedSkills`.
3. Focus on data aggregation, communication, and process management.
4. Ensure the resulting skill set is lightweight and focused on efficiency.

## Skill Set

### 🔄 Coordination & Orchestration
- **Task Scheduling**: Managing the sequence of task execution and dependency resolution.
- **Agent Handoff**: Ensuring smooth transitions of context and responsibility between different roles.
- **Status Reporting**: Aggregating progress from multiple agents into a concise summary.

### 📑 Information Synthesis
- **Data Collation**: Gathering disparate pieces of information into a structured format.
- **Summary Generation**: Distilling complex technical discussions into key takeaways and action items.
- **Artifact Mapping**: Ensuring that tasks, bugs, and features are correctly linked in the store.

### 🛠️ Basic Tooling
- **File Management**: Basic use of `Read`, `Write`, and `Glob` for housekeeping.
- **Git Basics**: Performing simple commits and status checks.
- **Log Monitoring**: Watching for specific event patterns to trigger transitions.

## Orchestrator Iron Laws

These laws apply to every orchestrator workflow (task pipeline and bug-fix pipeline). They are the non-negotiable invariants of the phase loop.

**YOU MUST NOT advance a phase until its gate checks pass.** Skipping a gate because "it's probably fine" or "it's a small change" is not allowed. No exceptions.

**Review ordering is hardcoded:** spec compliance review ALWAYS runs before code quality review. Never reverse this. Checking quality before confirming correctness is wasted work.

**Revision loop exhaustion is an escalation trigger.** If max_iterations is reached without approval, escalate to the human immediately. Do NOT approve to unblock the pipeline.

**Always read the verdict from the artifact.** Never assume approval because the review phase ran without error. The artifact is the source of truth.

**Phase banners are orchestrator-owned.** Do NOT include banner-first instructions in subagent prompts. The orchestrator displays the badge before spawning and the exit signal after return.

**No emoji in machine-readable fields.** Emoji belong only in stdout announcements and human-facing Markdown. JSON fields use plain values only.
