---
name: run-task
description: Orchestrate the full plan→implement→review→approve pipeline for a task
---

# /forge:run-task

Read the run-task workflow and follow it.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Execute

workflow('wfl:run-task', $ARGUMENTS)

## Arguments

$ARGUMENTS