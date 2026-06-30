---
id: bug-fixer-skills
name: Bug-Fixer Meta-Skills
description: Core capabilities and toolsets for the Bug-Fixer role.
role: BugFixer
applies_to: [bug-fixer]
summary: >
  Rapid reproduction, isolation, surgical remediation, and regression-safe
  verification of reported bugs.
capabilities:
  - Create minimal reproducible examples
  - Bisect commits and analyse logs to locate failure
  - Apply surgical fixes that avoid collateral damage
  - Write a regression test that fails without the fix and passes with it
  - Verify fixes across environments and under stress
file_ref: .forge/skills/bug-fixer-skills.md
---

## Generation Instructions

When generating the project-specific skill set for the Bug-Fixer role in `.forge/skills/bug-fixer-skills.md`, the generator must:
1. Cross-reference the `installedSkills` list in `.forge/config.json`.
2. Map the universal skills listed below to the specific implementation names found in `installedSkills`.
3. Emphasize tools for rapid reproduction, isolation, and verification.
4. Ensure the resulting skill set focuses on minimizing regression and maximizing fix stability.

## Skill Set

### 🐛 Triage & Isolation
- **Reproduction**: Creating minimal, reproducible examples of the reported bug.
- **Log Analysis**: Sifting through system and application logs to identify the point of failure.
- **State Inspection**: Using debuggers or telemetry to examine the system state at the moment of the crash.
- **Bisection**: Using git bisect or similar techniques to find the commit that introduced the bug.

### 🛠️ Targeted Remediation
- **Surgical Fixes**: Applying the most precise fix possible to avoid collateral damage.
- **Regression Prevention**: Writing a specific test case that fails without the fix and passes with it.
- **Hotfix Deployment**: Managing the rapid release of critical fixes to production environments.

### 🧪 Verification & Validation
- **Stress Testing**: Subjecting the fix to high loads or unusual inputs to ensure stability.
- **Cross-Environment Testing**: Verifying the fix across different OSs, browsers, or hardware configurations.
- **Verification Sign-off**: Providing evidence that the bug is resolved and no new issues were introduced.
