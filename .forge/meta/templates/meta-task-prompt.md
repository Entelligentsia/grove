# Meta-Template: Task Prompt

## Purpose

Defines the structure of a task prompt — the input document that starts
every task through the pipeline.

## Sections

### Required
- **Title** — one-line summary
- **Objective** — what the task achieves (user-facing value)
- **Acceptance Criteria** — numbered, testable conditions for completion
- **Context** — relevant background, links to related tasks/bugs

### Optional (added based on stack detection)
- **Entities** — which business entities are involved
- **API Changes** — new/modified endpoints (if API project)
- **UI Changes** — new/modified components (if frontend project)
- **Data Model Changes** — schema/migration needs
- **Operational Impact** — deployment, monitoring, rollback considerations

## Generation Instructions
- Add entity references from the project's business domain
- Add stack-specific acceptance criteria patterns
- Include the project's ID format in the template header
