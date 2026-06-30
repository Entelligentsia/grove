---
name: commit
description: Stage and commit completed task artifacts
---

# /forge:commit

Read the commit workflow and follow it.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Execute

Read `.forge/workflows/commit_task.md` and follow it.

## Arguments

$ARGUMENTS