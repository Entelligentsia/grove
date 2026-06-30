---
id: architect-skills
name: Architect Meta-Skills
description: Core capabilities and toolsets for the Architect role.
role: Architect
applies_to: [architect]
summary: >
  High-level system design, strategic planning, and architecture review
  capabilities that prioritise scalability, maintainability, and integrity.
capabilities:
  - Evaluate system structure for debt and bottlenecks
  - Select design patterns and define interface contracts
  - Map technical roadmaps across sprints
  - Perform trade-off and scalability analysis
  - Review implementations for architectural drift
file_ref: .forge/skills/architect-skills.md
---

## Generation Instructions

When generating the project-specific skill set for the Architect role in `.forge/skills/architect-skills.md`, the generator must:
1. Cross-reference the `installedSkills` list in `.forge/config.json`.
2. Map the universal skills listed below to the specific implementation names found in `installedSkills`.
3. Focus on high-level analysis, system design, and strategic planning tools.
4. Ensure the resulting skill set prioritizes scalability, maintainability, and architectural integrity.

## Skill Set

### 🏗️ System Design & Modeling
- **Architecture Analysis**: Evaluating the current system structure to identify technical debt and bottlenecks.
- **Design Pattern Selection**: Determining the most appropriate patterns (e.g., Microservices, Event-driven) for new features.
- **Data Modeling**: Designing efficient database schemas and data flow diagrams.
- **Interface Specification**: Defining clear API contracts and communication protocols between components.

### 🗺️ Strategic Planning
- **Technical Roadmap**: Mapping out the evolution of the system over multiple sprints.
- **Trade-off Analysis**: Weighing the pros and cons of different technical approaches (e.g., Build vs. Buy).
- **Complexity Management**: Breaking down large architectural goals into manageable technical tasks.

### 🔍 High-Level Review
- **Architecture Review**: Ensuring that implementations align with the intended design and don't introduce "architectural drift".
- **Scalability Assessment**: Analyzing how the system will handle growth in users or data volume.
- **Security Modeling**: Identifying potential attack vectors and designing mitigation strategies.
