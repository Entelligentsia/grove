# Role-to-Workflow Mapping

This document defines the mapping between the Forge personas and the meta-workflows they are responsible for executing. This mapping ensures that generated project workflows are assigned to the correct agent role.

| Persona | Symbol | Meta-Workflows |
| :--- | :---: | :--- |
| **Product Manager** | 📋 | `meta-sprint-intake.md` |
| **Architect** | ⛰️ | `meta-sprint-plan.md`, `meta-approve.md` |
| **Orchestrator** | 🌊 | `meta-orchestrate.md`, `meta-fix-bug.md` |
| **Engineer** | 🌱 | `meta-plan-task.md`, `meta-implement.md`, `meta-update-plan.md`, `meta-update-implementation.md`, `meta-commit.md` |
| **Supervisor** | 🌿 | `meta-review-plan.md`, `meta-review-implementation.md` |
| **QA Engineer** | 🧪 | `meta-validate.md` |
| **Collator** | 🍃 | `meta-collate.md` |
| **Retrospective Agent** | 🌀 | `meta-retrospective.md`, `meta-review-sprint-completion.md` |

*Note: The Retrospective Agent is often a specialization of the Architect or a dedicated audit role. In the meta-definitions, these workflows are mapped to the retrospective cycle.*
