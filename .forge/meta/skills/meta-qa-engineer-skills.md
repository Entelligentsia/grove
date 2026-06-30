---
id: qa-engineer-skills
name: QA Engineer Meta-Skills
description: Core capabilities and toolsets for the QA Engineer role.
role: QAEngineer
applies_to: [qa-engineer]
summary: >
  Test strategy, coverage analysis, and verification that prevents
  regressions and validates acceptance criteria against implementations.
capabilities:
  - Design test plans mapping each acceptance criterion to test cases
  - Analyse coverage reports and identify untested paths
  - Probe edge cases, boundary conditions, and unusual inputs
  - Detect flaky tests and enforce quality gates
file_ref: .forge/skills/qa-engineer-skills.md
---

## Generation Instructions

When generating the project-specific skill set for the QA Engineer role in `.forge/skills/qa-engineer-skills.md`, the generator must:
1. Cross-reference the `installedSkills` list in `.forge/config.json`.
2. Map the universal skills listed below to the specific implementation names found in `installedSkills`.
3. Emphasize tools for test strategy, coverage analysis, and verification.
4. Ensure the resulting skill set focuses on preventing regressions and validating acceptance criteria.

## Skill Set

### 🧪 Test Strategy & Design
- **Test Plan Creation**: Designing comprehensive test plans that cover functional, integration, and edge-case scenarios.
- **Acceptance Criteria Validation**: Mapping each acceptance criterion to specific test cases that prove compliance.
- **Risk-Based Testing**: Prioritizing test effort on areas with the highest defect probability or business impact.

### 📊 Coverage & Analysis
- **Coverage Analysis**: Interpreting code coverage reports and identifying untested paths.
- **Gap Identification**: Finding scenarios not covered by the existing test suite and proposing new tests.
- **Regression Risk Assessment**: Evaluating the blast radius of code changes to determine regression risk.

### ✅ Verification & Validation
- **Build Verification**: Running the project's build and test commands to confirm that implementation meets specifications.
- **Specification Compliance**: Checking that the implementation matches the approved plan's acceptance criteria.
- **Edge Case Discovery**: Probing boundary conditions, error paths, and unusual input combinations.

### 🔄 Continuous Quality
- **Test Maintenance**: Keeping the test suite current as the codebase evolves.
- **Flakiness Detection**: Identifying and resolving non-deterministic test failures.
- **Quality Gates**: Enforcing test pass requirements before marking tasks as complete.