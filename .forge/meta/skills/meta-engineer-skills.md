---
id: engineer-skills
name: Engineer Meta-Skills
description: Core capabilities and toolsets for the Engineer role.
role: Engineer
applies_to: [engineer]
summary: >
  Concrete capabilities the Engineer persona must use to execute a task:
  code analysis, implementation, testing, debugging, and CI/CD alignment.
capabilities:
  - Analyse codebases using Grep, Read, Glob
  - Implement features per approved plans and project conventions
  - Write unit, integration, and end-to-end tests
  - Perform root cause analysis and impact analysis
  - Align commits and branches with the project's git flow
file_ref: .forge/skills/engineer-skills.md
---

## Generation Instructions

When generating the project-specific skill set for the Engineer role in `.forge/skills/engineer-skills.md`, the generator must:
1. Cross-reference the `installedSkills` list in `.forge/config.json`.
2. Map the universal skills listed below to the specific implementation names found in `installedSkills`.
3. Interpolate any project-specific tool names or internal CLI commands used for build, test, and deployment.
4. Ensure that the resulting skill set is actionable, providing clear triggers for when each skill should be invoked.

## Skill Set

### 🛠️ Implementation & Coding
- **Code Analysis**: Ability to read, analyze, and understand existing codebases using `Grep`, `Read`, and `Glob`.
- **Feature Implementation**: Converting technical designs into working code while adhering to project style guides.
- **Refactoring**: Improving code structure without altering behavior, focusing on maintainability and efficiency.
- **Test Writing**: Implementing unit, integration, and end-to-end tests to ensure correctness.

### 🔍 Investigation & Debugging
- **Root Cause Analysis**: Using logs, debugger tools, and hypothesis testing to isolate bugs.
- **Impact Analysis**: Assessing how a change in one part of the system affects other components.
- **Performance Profiling**: Identifying bottlenecks and optimizing critical paths.

### ⚙️ Workflow Integration
- **Git Mastery**: Managing branches, commits, and PRs following the project's git flow.
- **CI/CD Alignment**: Ensuring code passes pipeline checks and is deployable.
- **Tool Synthesis**: Creating small scripts or tools to automate repetitive engineering tasks.
