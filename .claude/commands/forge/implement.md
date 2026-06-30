---
name: implement
description: Execute the approved implementation plan for a task
---

# /forge:implement

Read the implement workflow and follow it.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Execute

Read `.forge/workflows/implement_plan.md` and follow it.

## Arguments

$ARGUMENTS