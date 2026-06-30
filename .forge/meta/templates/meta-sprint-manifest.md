# Meta-Template: Sprint Manifest

## Purpose

Defines the structure of the sprint planning output — the task breakdown,
estimates, and dependency graph for a sprint.

## Sections

### Required
- **Sprint ID** — using project prefix format
- **Sprint Title** — descriptive name
- **Goals** — what this sprint achieves
- **Task Table** — ID, title, estimate, dependencies, status
- **Dependency Graph** — Mermaid diagram showing task relationships
- **Execution Mode** — sequential / wave-parallel / full-parallel

### Optional
- **Risks** — known risks for this sprint
- **Carry-Over** — items from previous sprint
- **Technical Debt** — planned debt repayment

## Generation Instructions
- Use the project's ID format (PREFIX-S{NN}-T{NN})
- Reference the project's entity model for task scoping
- Include the project's operational impact categories
