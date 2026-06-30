---
name: run-sprint
description: Execute all tasks in a sprint (sequential or parallel)
---

# /forge:run-sprint

Read the run-sprint workflow and follow it.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Execute

workflow('wfl:run-sprint', $ARGUMENTS)

## Arguments

$ARGUMENTS