---
name: enhance
description: Progressive project-specific enrichment of structural elements
---

# /forge:enhance

Run the enhancement agent to enrich structural elements with project-specific knowledge.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Purpose

The enhancement agent observes the project and enriches structural elements
over time. It makes absolutely essential minimal modifications, preferring
runtime mix-in (KB references, `{{placeholder}}` substitution, `project-context.json`)
over modifying base artifacts.

## Behavior

1. Read `structure-versions.json` to determine current overlay level
2. Read `project-context.json` for current project specifics
3. Read KB changes since last enhancement
4. Scan codebase for patterns discovered since last enhancement
5. Compare current `.forge/` artifacts against base-pack baseline
6. For each artifact, determine what project-specific enrichment is warranted
7. Apply the minimal modification principle
8. Propose diffs (Phase 2+) or auto-apply (Phase 1)
9. Create new overlay version in `structure-versions.json` if needed

## Arguments

$ARGUMENTS