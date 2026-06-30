# Meta-Template: Implementation Plan

## Purpose

Defines the structure of PLAN.md — the Engineer's technical approach
document, reviewed by the Supervisor before implementation begins.

## Sections

### Required
- **Objective** — what this plan achieves
- **Approach** — high-level strategy
- **Files to Modify** — list of files with rationale
- **Testing Strategy** — what tests to write/modify
- **Acceptance Criteria** — copied from task prompt, mapped to implementation steps
- **Operational Impact** — deployment, migrations, monitoring

### Stack-Specific (added based on detection)
- **Data Model** subsections per framework (Django Models, Prisma Schema, etc.)
- **API Layer** subsections (DRF Serializers/Views, Express Routes, etc.)
- **Frontend** subsections (React Components, Vue Components, etc.)
- **Task Queue** subsections (Celery Tasks, Sidekiq Jobs, etc.)
- **Database Migrations** with framework-specific verification command

## Generation Instructions
- Add framework-specific subsections based on detected stack
- Test evidence section should expect output format of project's test runner
- Include the project's migration check command if applicable
