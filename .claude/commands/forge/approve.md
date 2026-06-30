---
name: approve
description: Final architect approval gate for a completed task
---

# /forge:approve

Read the approve workflow and follow it.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Execute

Read `.forge/workflows/architect_approve.md` and follow it.

## Arguments

$ARGUMENTS