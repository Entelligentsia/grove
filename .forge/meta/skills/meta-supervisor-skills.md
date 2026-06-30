---
id: supervisor-skills
name: Supervisor Meta-Skills
description: Core capabilities and toolsets for the Supervisor and QA role.
role: Supervisor
applies_to: [supervisor]
summary: >
  Quality assurance, review governance, and defect feedback — verifying
  specifications, test coverage, and compliance before work advances.
capabilities:
  - Validate requirements against implementations
  - Review test plans for coverage and effectiveness
  - Conduct code reviews for logic, style, and maintainability
  - Maintain audit trails and compliance checks
  - Orchestrate approval transitions from implementing to review-approved
file_ref: .forge/skills/supervisor-skills.md
---

## Generation Instructions

When generating the project-specific skill set for the Supervisor role in `.forge/skills/supervisor-skills.md`, the generator must:
1. Cross-reference the `installedSkills` list in `.forge/config.json`.
2. Map the universal skills listed below to the specific implementation names found in `installedSkills`.
3. Include triggers for quality gates, review cycles, and validation checks.
4. Ensure the resulting skill set emphasizes verification, correctness, and adherence to specifications.

## Skill Set

### ✅ Quality Assurance & Verification
- **Requirement Validation**: Comparing implemented features against the original specifications and acceptance criteria.
- **Test Plan Review**: Evaluating the coverage and effectiveness of the Engineer's test suite.
- **Edge Case Discovery**: Identifying potential failure modes and boundary conditions that may have been missed.
- **Regression Testing**: Ensuring that new changes do not break existing functionality.

### 📋 Review & Governance
- **Code Review**: Conducting thorough reviews of PRs for logic, style, and maintainability.
- **Audit Trails**: Ensuring that all changes are documented and linked to the appropriate tasks/bugs.
- **Compliance Checking**: Verifying that the code adheres to organizational standards and security policies.

### 📈 Feedback & Coordination
- **Defect Reporting**: Clearly documenting bugs and assigning them back to engineers with reproducible steps.
- **Progress Tracking**: Monitoring task completion and identifying blockers in the pipeline.
- **Approval Orchestration**: Managing the transition of tasks from `implementing` to `review-approved`.
