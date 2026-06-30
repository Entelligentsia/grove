---
id: store-custodian-skills
name: Store Custodian Skills
description: Deterministic store management — read, write, validate, and maintain the JSON flat-file store.
role: Collator
applies_to: [collator]
summary: >
  Deterministic store management capabilities — reading, writing, validating,
  and maintaining the JSON flat-file store with schema enforcement and
  referential integrity.
capabilities:
  - Read and write store records via the CLI
  - Validate store integrity with schema checks
  - Manage entity lifecycle (create, update, status transitions)
  - Merge sidecar data into canonical event records
  - Maintain referential integrity across tasks, sprints, bugs, and features
---

# {{PROJECT_NAME}} Store Custodian Skills

## 📦 Store Operations

- **CLI Interface**: Using `store-cli.cjs` for deterministic read, write, and status operations.
- **Schema Validation**: Running `validate-store.cjs` to check referential integrity and schema compliance.
- **Entity Lifecycle**: Creating tasks, sprints, bugs, and features with proper ID allocation and cross-references.

## 🔗 Referential Integrity

- **Foreign Key Checks**: Ensuring every `feature_id`, `sprintId`, and `taskId` references an existing entity.
- **Status Transitions**: Enforcing valid status transitions per entity type.
- **Ghost Event Handling**: Properly handling events that reference non-existent entities.

## ✍️ Writeback Discipline

- **Write-Boundary Contract**: All writes to store paths are schema-validated at the filesystem boundary.
- **Sidecar Merging**: Merging token usage sidecars into canonical event records atomically.
- **Collation State**: Recording what was collated, when, and with what hash.