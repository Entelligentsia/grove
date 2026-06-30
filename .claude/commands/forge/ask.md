---
name: ask
description: Ask Forge anything — project status, config, workflows, commands, version, or how-to questions
---

# /forge:ask

Single conversational entry point for all Forge intent.

## Locate plugin root

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Open with oracle banner

```sh
node "$FORGE_ROOT/tools/banners.cjs" oracle
node "$FORGE_ROOT/tools/banners.cjs" --subtitle "🏮 灯 Tomoshibi — your Forge concierge"
```

## Soft config check

Check whether `.forge/config.json` exists in the current working directory.
Project-status and config queries require it; Forge KB and workflow questions do not.
Store the result as `HAS_CONFIG` — pass it through to the oracle so it can handle
queries gracefully when config is absent.

## Invoke oracle

Read `$FORGE_ROOT/agents/tomoshibi.md` and follow it exactly.
The user's question is: $ARGUMENTS

## On error

If any step above fails unexpectedly, describe what went wrong and ask:

> "This looks like a Forge bug. Would you like to file a report to help improve it? Run `/forge:report-bug` — I'll pre-fill the report from this conversation."

## Arguments

$ARGUMENTS
