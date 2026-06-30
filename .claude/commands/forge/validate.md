---
name: validate
description: Validate that a task implementation satisfies acceptance criteria
---

# /forge:validate

Read the validate workflow and follow it.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Execute

Read `.forge/workflows/validate_task.md` and follow it.

## Arguments

$ARGUMENTS