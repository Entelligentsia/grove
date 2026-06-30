---
name: check-agent
description: Verify an agent has loaded and understood the project knowledge base
---

# /forge:check-agent

Read the check-agent workflow and follow it.

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Execute

Read `.forge/workflows/quiz_agent.md` and follow it.

## Arguments

$ARGUMENTS