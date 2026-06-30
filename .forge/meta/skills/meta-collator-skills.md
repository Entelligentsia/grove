---
id: collator-skills
name: Collator Meta-Skills
description: Core capabilities and toolsets for the Collator role.
role: Collator
applies_to: [collator]
summary: >
  Deterministic data aggregation, markdown regeneration, and store
  consistency — accurate writeback, entity linking, and index maintenance.
capabilities:
  - Regenerate MASTER_INDEX.md, TIMESHEET.md, and sprint summaries
  - Cross-reference tasks, bugs, features, events in the store
  - Detect referential integrity gaps and schema drift
  - Merge multi-source subagent outputs atomically
file_ref: .forge/skills/collator-skills.md
---

## Generation Instructions

When generating the project-specific skill set for the Collator role in `.forge/skills/collator-skills.md`, the generator must:
1. Cross-reference the `installedSkills` list in `.forge/config.json`.
2. Map the universal skills listed below to the specific implementation names found in `installedSkills`.
3. Emphasize tools for data aggregation, markdown regeneration, and store consistency.
4. Ensure the resulting skill set focuses on accurate writeback, entity linking, and index maintenance.

## Skill Set

### 📑 Data Aggregation & Writeback
- **Markdown Regeneration**: Rebuilding `MASTER_INDEX.md`, sprint summaries, and progress reports from the JSON store.
- **Entity Linking**: Ensuring tasks, bugs, features, and events are correctly cross-referenced in the store.
- **Store Writeback**: Persisting generated artifacts using `store-cli.cjs` via the Store Custodian skill.

### 🔍 Consistency & Validation
- **Referential Integrity**: Detecting orphaned entities, broken links, and stale references across store records.
- **Index Reconciliation**: Comparing `MASTER_INDEX.md` against the actual store contents and flagging drift.
- **Schema Compliance**: Validating that store records conform to their respective JSON schemas before writeback.

### 🔄 Synchronization
- **Multi-Source Merge**: Combining data from multiple subagent outputs into a single coherent artifact.
- **Change Detection**: Identifying which store records have been modified since the last collation pass.
- **Atomic Updates**: Ensuring that partial writeback failures do not leave the store in an inconsistent state.