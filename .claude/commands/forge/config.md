---
name: config
description: Inspect Forge project configuration.
---

# /forge:config

Read `.forge/config.json` values that are user-facing. Other config keys remain managed by their respective
commands (e.g. `paths.forgeRoot` is refreshed by `/forge:update`).

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

Read `.forge/config.json`. If it does not exist, stop and emit:

```
× .forge/config.json not found. Run /forge:init first.
```

Then exit. (`manage-config.cjs` already prints this same message on missing
config, so the user-visible behaviour is consistent if you only invoke the
tool.)

## Arguments

$ARGUMENTS

Parse the argument:

```
/forge:config                    # Print summary of .forge/config.json
```

The command shape is reserved for future expansion (e.g. `/forge:config kb
<path>`, `/forge:config paths <key> <value>`).

---

## Visual

For all subcommands, open with a single `north` badge (config = bearings):

```sh
node "$FORGE_ROOT/tools/banners.cjs" --badge north
```

`banners.cjs` strips ANSI in `NO_COLOR` / non-tty / `--plain` contexts.

---

## Subcommand: `/forge:config` (no args) — summary

Read `.forge/config.json`. Emit a summary block:

```
━━━ Forge Configuration ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  version:     {version}
  project:     {project.name} ({project.prefix})

  Paths
    engineering:   {paths.engineering}
    store:         {paths.store}
    workflows:     {paths.workflows}
    commands:      {paths.commands}
    templates:     {paths.templates}
    forgeRoot:     {paths.forgeRoot}

  Installed skills ({installedSkills.length})
    {one per line}
```

Use these tool invocations:

```sh
node "$FORGE_ROOT/tools/manage-config.cjs" get version
node "$FORGE_ROOT/tools/manage-config.cjs" get project
node "$FORGE_ROOT/tools/manage-config.cjs" get paths
node "$FORGE_ROOT/tools/manage-config.cjs" get installedSkills 2>/dev/null
```

This subcommand is **read-only** — it never writes to disk.

---

## On error

If any step above fails unexpectedly, describe what went wrong and ask:

> "This looks like a Forge bug. Would you like to file a report to help improve it? Run `/forge:report-bug` — I'll pre-fill the report from this conversation."
