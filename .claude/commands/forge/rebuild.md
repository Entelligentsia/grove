---
name: rebuild
description: Use when the engineering knowledge base has been enriched by sprints and you want to refresh the generated workflows, templates, tools, or knowledge-base docs. Use --enrich to run the enhancement agent (KB enrichment + drift detection).
---

# /forge:rebuild

Re-run generation phases using the current state of the project. Use `--enrich` to run the enhancement agent (replaces the removed `forge:enhance` command).

## Locate the Forge plugin

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

Read `.forge/config.json`. If it does not exist, stop and tell the user to run
`/forge:init` first.

Resolve tools from the plugin:
```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

All tool invocations in this command use `node "$FORGE_ROOT/tools/<tool>.cjs"`.

## Hero

Open the run with the forge hero + a one-line subtitle (parses
`$ARGUMENTS` to mention the target):

```sh
node "$FORGE_ROOT/tools/banners.cjs" forge
node "$FORGE_ROOT/tools/banners.cjs" --subtitle "Re-running generation against current meta-definitions ($ARGUMENTS or full)"
```

Each category section below opens with a `banners.cjs --badge {key}` call
before its "Generating ..." line. The badge map:

| Category | Banner key |
|----------|-----------|
| personas | `bloom` |
| skills | `tide` |
| templates | `drift` |
| workflows | `ember` |
| commands | `lumen` |
| knowledge-base | `oracle` |

`banners.cjs` strips ANSI in `NO_COLOR` / non-tty / `--plain` contexts.

## Arguments

$ARGUMENTS

Parse the argument to identify the target category and optional sub-target.
Sub-targets may be passed either as a second positional argument or embedded
with a colon delimiter (both forms are equivalent).

> **Category scope enforcement**: If this workflow (or any subagent it spawns)
> calls `substitute-placeholders.cjs` directly to re-materialise base-pack
> files, it MUST pass `--category <parsed-category>` to restrict writes to the
> requested namespace. Without this flag the tool overwrites all five output
> directories (personas, skills, workflows, templates, commands), silently
> discarding any regenerated content in non-targeted categories.
>
> Example:
> ```sh
> node "$FORGE_ROOT/tools/substitute-placeholders.cjs" \
>   --base-pack  "$FORGE_ROOT/init/base-pack" \
>   --config     ".forge/config.json" \
>   --category   "personas" \
>   --out        "."
> ```

```
/forge:rebuild                              # workflows + commands + templates + personas (default)
/forge:rebuild --enrich                    # run enhancement agent (KB enrichment + drift detection)
/forge:rebuild personas                    # .forge/personas/ — all persona files
/forge:rebuild personas engineer           # single persona file only
/forge:rebuild personas:engineer           # same — colon form (from migration entries)
/forge:rebuild skills                        # .forge/skills/ role-specific skills
/forge:rebuild skills engineer              # single skill file only
/forge:rebuild skills:engineer              # same — colon form (from migration entries)
/forge:rebuild workflows                    # full workflow rebuild
/forge:rebuild workflows plan_task          # single workflow file only
/forge:rebuild workflows:plan_task          # same — colon form (from migration entries)
/forge:rebuild workflows sprint_plan        # single workflow file only
/forge:rebuild workflows-js                  # .claude/workflows/ JS orchestration workflows (verbatim copy)
/forge:rebuild workflows-js wfl-run-task     # single JS workflow file only
/forge:rebuild workflows-js:wfl-run-task     # same — colon form (from migration entries)
/forge:rebuild tools                        # .forge/tools/ verbatim re-copy from $FORGE_ROOT/tools/
/forge:rebuild tools store-cli              # single tool file only (name with or without .cjs)
/forge:rebuild tools:store-cli              # same — colon form (from migration entries)
/forge:rebuild commands                     # .claude/commands/ slash command wrappers
/forge:rebuild templates                    # document templates only
/forge:rebuild templates PLAN_TEMPLATE      # single template file only
/forge:rebuild templates:PLAN_TEMPLATE      # same — colon form (from migration entries)
/forge:rebuild knowledge-base               # all three sub-targets (merge mode)
/forge:rebuild knowledge-base architecture
/forge:rebuild knowledge-base:architecture  # colon form (from migration entries)
/forge:rebuild knowledge-base business-domain
/forge:rebuild knowledge-base stack-checklist
```

When parsing the argument, split on `:` first: if the argument is
`"workflows:plan_task"`, treat it as category=`workflows`,
sub-target=`plan_task`. If no `:` is present, the second positional word
(if any) is the sub-target. The sub-target is always optional.

---

## Category: `personas` — full rebuild or single file

Re-generate `.forge/personas/` from the meta-persona definitions and the current knowledge base.

**If a sub-target is provided** (e.g. `/forge:rebuild personas engineer`
or the colon form `personas:engineer`), regenerate only the single persona
file `.forge/personas/<sub-target>.md` from `$FORGE_ROOT/meta/personas/meta-<sub-target>.md`.

Before writing, check the file for manual modifications (mirrors the workflows
and templates pre-write guard — FORGE-BUG-037 / forge#106):
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" check .forge/personas/<sub-target>.md
```
For exit 1 (modified): warn `△ .forge/personas/<sub-target>.md has been manually
modified (likely by /forge:enhance). Overwriting will discard your changes.
Proceed? (yes / no / show diff)`. Collect the answer before proceeding. On
`no` or `show diff` rejecting overwrite, skip this file and exit cleanly.
On exit 2 (untracked) or exit 3 (missing): proceed without prompting.

Then remove any existing manifest entry for this specific file (handles rename case):
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" remove .forge/personas/<sub-target>.md 2>/dev/null || true
```
Generate the single file (no fan-out needed for one file). All manifest and hash
steps below apply to that single file.

**If no sub-target** — full rebuild, fanned out in parallel:

1. Build the project brief:
   ```sh
   node "$FORGE_ROOT/tools/build-init-context.cjs" \
     --config .forge/config.json --personas .forge/personas \
     --templates .forge/templates --kb "$(node "$FORGE_ROOT/tools/manage-config.cjs" get paths.engineering 2>/dev/null || echo engineering)" \
     --out .forge/init-context.md --json-out .forge/init-context.json
   ```
2. Enumerate `$FORGE_ROOT/meta/personas/meta-*.md` (exclude README.md).
   Let `M_total` = the enumerated count.

3. Render the personas badge, then emit the count:
   ```sh
   node "$FORGE_ROOT/tools/banners.cjs" --badge bloom
   ```
   Then emit: `Generating personas (<N> files in parallel)...`
4. Check each enumerated file for manual modifications
   before any clearing or regeneration (mirrors the workflows + templates
   pre-write guard — FORGE-BUG-037 / forge#106):
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" check .forge/personas/<role>.md
   ```
   For any exit 1 (modified): warn `△ .forge/personas/<role>.md has been manually
   modified (likely by /forge:enhance). Overwriting will discard your changes.
   Proceed? (yes / no / show diff)`. Collect answers before proceeding. Files
   the user declines are removed from the regeneration set for this run. Exit
   2 (untracked) and exit 3 (missing) require no prompt.
5. Clear stale entries:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" clear-namespace .forge/personas/
   ```
6. **Spawn the persona subagents in a SINGLE Agent tool message** using
   `$FORGE_ROOT/init/generation/generate-persona.md` as the per-subagent rulebook
   (same fan-out pattern as `/forge:init` Phase 4). Spawn one per entry.
7. Collect results. For each `done:` result → emit `  〇 <filename>.md`.
   Retry failures once. Any still failing: surface the id list.
8. **Replay user enhancements** (forge#107 / Approach A — layer 3 of the composition
   contract declared at `manage-versions.cjs:13`). After fresh base-pack content
   is written, restore any user-enhanced files captured by `/forge:enhance` Phase 2
   snapshots:
   ```sh
   node "$FORGE_ROOT/tools/manage-versions.cjs" replay --target personas
   ```
   The tool walks all snapshots in `.forge/structure-versions.json`, finds enhanced
   elements whose normalized path starts with `personas/`, and copies them from
   the archive back over the freshly-generated content. Later snapshots win on
   file collision. Files not captured by any snapshot remain at the fresh
   base-pack version.
9. Re-record manifest hashes for the (now restored) files so subsequent
   `generation-manifest check` calls reflect current on-disk content:
   ```sh
   for each <role> in the full set:
     node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/personas/<role>.md
   ```
10. Emit `  〇 personas — <N> files written`.

---

## Category: `skills` — full rebuild or single file

Re-generate `.forge/skills/` from the meta-skill templates and project config.

**If a sub-target is provided** (e.g. `/forge:rebuild skills engineer`
or the colon form `skills:engineer`), regenerate only the single skill file
`.forge/skills/<sub-target>-skills.md` from
`$FORGE_ROOT/meta/skills/meta-<sub-target>-skills.md`.

Before writing, check the file for manual modifications (mirrors the workflows
and templates pre-write guard — FORGE-BUG-037 / forge#106):
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" check .forge/skills/<sub-target>-skills.md
```
For exit 1 (modified): warn `△ .forge/skills/<sub-target>-skills.md has been
manually modified (likely by /forge:enhance). Overwriting will discard your
changes. Proceed? (yes / no / show diff)`. Collect the answer before
proceeding. On `no` or `show diff` rejecting overwrite, skip this file and
exit cleanly. On exit 2 (untracked) or exit 3 (missing): proceed without
prompting.

Then remove any existing manifest entry for this specific file:
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" remove .forge/skills/<sub-target>-skills.md 2>/dev/null || true
```
Generate the single file (no fan-out needed). All manifest and hash steps below
apply to that single file.

**If no sub-target** — full rebuild, fanned out in parallel:

1. Build the project brief (same command as in `personas` above).
2. Enumerate `$FORGE_ROOT/meta/skills/meta-*-skills.md`. Let `M_total` =
   the enumerated count.

3. Render the skills badge, then emit the count:
   ```sh
   node "$FORGE_ROOT/tools/banners.cjs" --badge tide
   ```
   Then emit: `Generating skills (<N> files in parallel)...`
4. Check each enumerated file for manual modifications before any clearing
   or regeneration (mirrors the workflows + templates pre-write guard —
   FORGE-BUG-037 / forge#106):
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" check .forge/skills/<role>-skills.md
   ```
   For any exit 1 (modified): warn `△ .forge/skills/<role>-skills.md has been
   manually modified (likely by /forge:enhance). Overwriting will discard your
   changes. Proceed? (yes / no / show diff)`. Collect answers before proceeding.
   Files the user declines are removed from the regeneration set for this run.
   Exit 2 (untracked) and exit 3 (missing) require no prompt.
5. Clear stale entries:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" clear-namespace .forge/skills/
   ```
6. **Spawn the skill subagents in a SINGLE Agent tool message** using
   `$FORGE_ROOT/init/generation/generate-skill.md` as the per-subagent rulebook.
7. Collect results. Retry failures once. Any still failing: surface the id list.
8. **Replay user enhancements** (forge#107 / Approach A):
   ```sh
   node "$FORGE_ROOT/tools/manage-versions.cjs" replay --target skills
   ```
   Walks snapshots; restores any enhanced `skills/<role>-skills.md` files from
   the archive over the freshly-generated content. Later snapshots win on
   collision.
9. Re-record manifest hashes for the (now restored) files:
   ```sh
   for each <role> in the full set:
     node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/skills/<role>-skills.md
   ```
10. For each completed file, check manifest (warn on modified), emit `  〇 <filename>.md`.

---

## Category: `workflows` — full rebuild or single file

Re-generate `.forge/workflows/` from the meta-workflow definitions and the
current knowledge base. Covers both atomic workflows and orchestration.

**If a sub-target is provided** (e.g. `/forge:rebuild workflows plan_task`
or the colon form `workflows:plan_task`), regenerate only the single workflow
file `.forge/workflows/<sub-target>.md`.

**Special case — `_fragments` sub-target (directory fan-out):**

`_fragments` is a directory in `$FORGE_ROOT/meta/workflows/_fragments/`, not a
single file. When the sub-target is `_fragments` (or when
`$FORGE_ROOT/meta/workflows/<sub-target>` resolves to a directory rather than a
file), use directory fan-out mode instead of single-file mode:

1. Enumerate all `.md` files in `$FORGE_ROOT/meta/workflows/_fragments/`.
2. Ensure `.forge/workflows/_fragments/` directory exists (create if absent).
3. For each fragment file, copy verbatim (no placeholder substitution) to
   `.forge/workflows/_fragments/<filename>`.
4. Record a manifest hash for each copied file:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/workflows/_fragments/<filename>
   ```
5. Emit `〇 workflows:_fragments — N fragment files copied`.

> **Note:** When invoked as `/forge:rebuild workflows:_fragments`, this command
> fans out to copy all fragment files (currently four: `context-injection.md`,
> `progress-reporting.md`, `event-emission-schema.md`, `finalize.md`). Verify the
> result with `ls .forge/workflows/_fragments/`.

For all other sub-targets (non-directory), continue with the standard single-file
path below.

Before writing, remove any existing manifest entry for this specific file:
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" remove .forge/workflows/<sub-target>.md 2>/dev/null || true
```
Generate the single file (no fan-out needed). Check manifest (warn on modified),
write, record hash.

**If no sub-target** — full rebuild using the same parallel fan-out as `/forge:init` Phase 7:

1. Build the project brief:
   ```sh
   node "$FORGE_ROOT/tools/build-init-context.cjs" \
     --config .forge/config.json --personas .forge/personas \
     --templates .forge/templates \
     --kb "$(node "$FORGE_ROOT/tools/manage-config.cjs" get paths.engineering 2>/dev/null || echo engineering)" \
     --out .forge/init-context.md --json-out .forge/init-context.json
   ```
2. Read `$FORGE_ROOT/init/workflow-gen-plan.json` (15-entry fan-out table).
   Let `M_total` = the entry count.

3. Render the workflows badge, then emit the count:
   ```sh
   node "$FORGE_ROOT/tools/banners.cjs" --badge ember
   ```
   Then emit: `Generating workflows (<N> atomic, parallel)...`
4. Check each file for manual modifications before any clearing:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" check .forge/workflows/{filename}.md
   ```
   For any exit 1 (modified): warn `△ .forge/workflows/{filename}.md has been manually
   modified. Overwriting may discard manual edits not captured in any /forge:enhance snapshot. Edits captured by snapshots will be restored automatically via `manage-versions replay` after regeneration. Proceed? (yes / no / show diff)`
   Collect answers before proceeding.
5. Clear stale entries:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" clear-namespace .forge/workflows/
   ```
6. **Spawn the atomic workflow subagents in a SINGLE Agent tool message** using
   `$FORGE_ROOT/init/generation/generate-workflows.md` as the per-subagent rulebook
   (same fan-out pattern as `/forge:init` Phase 7d). Spawn one per entry.
7. Collect results. Retry failures once in a single Agent call.

   > **LLM orchestration retired.** `orchestrate_task` / `run_sprint` / `fix_bug`
   > are no longer generated. The deterministic JS drivers in
   > `.claude/workflows/wfl-*.js` (category `workflows-js`) are the only
   > orchestration truth; `/forge:run-task`, `/forge:run-sprint`, and
   > `/forge:fix-bug` dispatch to them via `workflow(wfl:*)`. The prose specs
   > `meta-orchestrate.md` / `meta-fix-bug.md` remain in `meta/` as reference
   > docs only — neither built into the base-pack nor regenerated here.

8. **Replay user enhancements** (forge#107 / Approach A):
   ```sh
   node "$FORGE_ROOT/tools/manage-versions.cjs" replay --target workflows
   ```
   Walks snapshots; restores enhanced `workflows/<name>.md` files. Later
   snapshots win on collision.
9. For each written file: record hash `node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/workflows/{filename}.md`
    (this runs AFTER replay so the recorded hash reflects the restored content).
10. Emit `  〇 workflows — <N> files written`.

**Do NOT touch:** `.claude/commands/`, `.forge/config.json`, or any knowledge base file.

---

## Category: `workflows-js` — verbatim copy (full or single file)

Re-materialise the JS orchestration workflows in `.claude/workflows/` from the
plugin base-pack. Unlike the `workflows` category (LLM-generated `.forge/`
markdown with placeholder substitution and KB enrichment), the
`workflows-js` files are **deterministic verbatim copies** from
`$FORGE_ROOT/init/base-pack/workflows-js/` — no LLM, no placeholder
substitution, no enrichment. The output is byte-identical to the base-pack
source (enforced by `workflows-js-drift.test.cjs`), so this category mirrors
the `workflows:_fragments` copy pattern, not the generation pattern.

Render a badge, then proceed:

```sh
node "$FORGE_ROOT/tools/banners.cjs" --badge ember
```

**If a sub-target is provided** (e.g. `/forge:rebuild workflows-js wfl-run-task`
or the colon form `workflows-js:wfl-run-task`), copy only the single file. The
sub-target may be given with or without the `.js` extension; normalise to
`<sub-target>.js`.

1. Verify `$FORGE_ROOT/init/base-pack/workflows-js/<sub-target>.js` exists. If
   not, list the available files and exit cleanly.
2. Ensure `.claude/workflows/` exists (create if absent).
3. Copy verbatim (no substitution) to `.claude/workflows/<sub-target>.js`.
4. Record a manifest hash:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record .claude/workflows/<sub-target>.js
   ```
5. Emit `  〇 workflows-js:<sub-target> — copied`.

**If no sub-target** — full copy, directory fan-out:

1. Enumerate all `.js` files in `$FORGE_ROOT/init/base-pack/workflows-js/`.
   Let `N` = the count.
2. Emit: `Copying workflows-js (<N> files)...`
3. Ensure `.claude/workflows/` exists (create if absent).
4. For each file, copy verbatim to `.claude/workflows/<filename>`, then record
   a manifest hash:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record .claude/workflows/<filename>
   ```
5. Emit `  〇 workflows-js — <N> files copied`.

> **Note:** Only the base-pack-sourced JS workflows (`wfl-*.js`) are written.
> Custom or project-specific files in `.claude/workflows/` are never
> overwritten or deleted. Verify with `ls .claude/workflows/`.

---

## Category: `tools` — verbatim copy (full or single file)

Re-materialise the vendored plugin tools in `.forge/tools/` from the installed
plugin. Unlike workflow and persona categories (LLM-generated with placeholder
substitution), tools files are **deterministic verbatim copies** from
`$FORGE_ROOT/tools/` — no LLM, no substitution, no enrichment. The output is
byte-identical to the plugin source.

**If a sub-target is provided** (e.g. `/forge:rebuild tools store-cli`
or the colon form `tools:store-cli`), copy only the single file. The sub-target
may be given with or without the `.cjs` extension; normalise to `<sub-target>.cjs`.
Sub-targets in the `lib/` namespace may be specified with the `lib/` prefix
(e.g. `tools:lib/schema-loader` or `tools:lib/schema-loader.cjs`).

**Single-file steps:**

1. Resolve the source path: if the sub-target starts with `lib/`, look in
   `$FORGE_ROOT/tools/lib/<name>.cjs`; otherwise look in `$FORGE_ROOT/tools/<name>.cjs`.
   If the source does not exist, list the available files and exit cleanly.
2. Ensure the target directory exists (`.forge/tools/` or `.forge/tools/lib/`).
3. Copy verbatim to the resolved `.forge/tools/` path.
4. Record a manifest hash:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/tools/<sub-target>.cjs
   ```
5. Emit `  〇 tools:<sub-target> — copied`.

**If no sub-target** — full re-copy, directory fan-out:

1. Enumerate all `*.cjs` **and `*.js`** files in `$FORGE_ROOT/tools/` (top-level
   only, exclude `*.test.cjs`/`*.test.js`). Let `N_top` = the count.
   Enumerate all `*.cjs` **and `*.js`** files in `$FORGE_ROOT/tools/lib/` (exclude
   `*.test.cjs`/`*.test.js`). Let `N_lib` = the count.
   Both extensions are required: e.g. `store-cli.cjs` loads `lib/validate.js` and
   `collate.cjs` loads `lib/result.js`; a `.cjs`-only copy breaks them.
2. Emit: `Copying tools (<N_top> tool files + <N_lib> lib files)...`
3. Ensure `.forge/tools/` and `.forge/tools/lib/` exist (create if absent).
4. For each top-level file, copy verbatim to `.forge/tools/<filename>`, then
   record a manifest hash:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/tools/<filename>
   ```
5. For each lib file, copy verbatim to `.forge/tools/lib/<filename>`, then
   record a manifest hash:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/tools/lib/<filename>
   ```
6. Write (or overwrite) the version marker so `/forge:health` can detect staleness:
   ```sh
   ACTIVE_VERSION=$(node -e "console.log(require('$FORGE_ROOT/.claude-plugin/plugin.json').version)")
   node -e "
   const fs = require('fs');
   fs.writeFileSync('.forge/tools/.forge-tools-version', JSON.stringify({ version: '${ACTIVE_VERSION}' }) + '\n');
   "
   ```
7. Emit `  〇 tools — <N_top + N_lib> files copied`.

> **Note:** This is a full re-copy of the plugin tools at the installed
> `$FORGE_ROOT` version. Use `/forge:rebuild tools` after `/forge:update` to
> refresh `.forge/tools/` to the new plugin version.

---

## Category: `commands` — full rebuild

Re-generate `.claude/commands/` slash command wrappers from the current
`.forge/workflows/`. This is a thin generation step — each command file
is just a wrapper that loads its workflow and passes arguments.

Run this when:
- Workflow files have been renamed (e.g. after a 0.5.0 upgrade)
- A new workflow was added and its command wrapper is missing
- A command wrapper is pointing at a workflow that no longer exists

1. Read `.forge/config.json` for paths
2. Enumerate `.forge/workflows/` to know what workflow files currently exist
3. Render the commands badge, then emit the count:
   ```sh
   node "$FORGE_ROOT/tools/banners.cjs" --badge lumen
   ```
   Then emit: `Generating commands (<N> files)...`
4. Clear stale entries for this namespace:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" clear-namespace .claude/commands/
   ```
5. Re-generate `.claude/commands/` following
   `$FORGE_ROOT/init/generation/generate-commands.md`
   The idempotency check will overwrite any command that references a
   missing or renamed workflow, and skip any that are already correct.
6. For each file being written:
   - Emit: `  ⋯ <filename>.md...`
   - Write the file
   - Record hash: `node "$FORGE_ROOT/tools/generation-manifest.cjs" record .claude/commands/{filename}.md`
   - Emit: `  〇 <filename>.md`

**DO NOT** touch any `.claude/commands/` file that is not in the output list
in `generate-commands.md`. Custom commands (`supervisor-code.md`, project-specific
wrappers, etc.) must never be written, overwritten, or deleted by this step.
Retired Forge command files (`engineer.md`, `supervisor.md`) are cleaned up
separately by Step 5b-pre in `/forge:update` — not here.

---

## Category: `templates` — full rebuild or single file

Re-generate `.forge/templates/` from the meta-template definitions and the
current knowledge base.

**If a sub-target is provided** (e.g. `/forge:rebuild templates PLAN_TEMPLATE`
or the colon form `templates:PLAN_TEMPLATE`), regenerate only the single template
file `.forge/templates/<sub-target>.md`. Determine the source meta file from
`$FORGE_ROOT/init/generation/generate-template.md`'s filename mapping (e.g.
`PLAN_TEMPLATE` → `meta-plan.md`).

Before writing, remove any existing manifest entry:
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" remove .forge/templates/<sub-target>.md 2>/dev/null || true
```
Generate the single file (no fan-out needed). Record hash after writing.

**If no sub-target** — full rebuild, fanned out in parallel:

1. Build the project brief (same command as in `personas` above).
2. Enumerate `$FORGE_ROOT/meta/templates/meta-*.md`. Let `M_total` =
   the enumerated count.

3. Render the templates badge, then emit the count:
   ```sh
   node "$FORGE_ROOT/tools/banners.cjs" --badge drift
   ```
   Then emit: `Generating templates (<N> files in parallel)...`
4. Check each (filtered) file for manual modifications (warn on modified, same pattern as workflows).
5. Clear stale entries:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" clear-namespace .forge/templates/
   ```
6. **Spawn the template subagents in a SINGLE Agent tool message** using
   `$FORGE_ROOT/init/generation/generate-template.md` as the per-subagent rulebook.
7. Collect results. Retry failures once. Any still failing: surface the id list.
8. **Replay user enhancements** (forge#107 / Approach A):
   ```sh
   node "$FORGE_ROOT/tools/manage-versions.cjs" replay --target templates
   ```
   Walks snapshots; restores enhanced `templates/<STEM>.md` files. Later
   snapshots win on collision.
9. For each written file: record hash, emit `  〇 <filename>.md` (hash reflects
   post-replay content).
10. Re-record the one-shot init artifact not regenerated from a meta file:
    ```sh
    if [ -f ".forge/templates/CUSTOM_COMMAND_TEMPLATE.md" ]; then
      node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/templates/CUSTOM_COMMAND_TEMPLATE.md
    fi
    ```
    Emit `〇 templates — <N> files written`.

---

## Category: `knowledge-base` — merge mode

**This is not a full rebuild.** The knowledge base accumulates writeback from
every sprint. Overwriting it from scratch destroys that accumulated knowledge.

Render the knowledge-base badge, then emit the status line:

```sh
node "$FORGE_ROOT/tools/banners.cjs" --badge oracle
```

Then emit: `Generating knowledge-base...`

Instead: re-run the relevant discovery prompts scoped to what has changed,
compute a delta against the existing docs, and merge only new content in.

**Per sub-target, emit merge-level status lines (not per-file):**

```
  ⋯ merging <sub-target> docs...
  〇 <sub-target> — N additions
```

**Merge rule (applies to all sub-targets):**
- Additive only — never remove or overwrite existing sections or entries.
- `[?]` markers from prior generation may be updated if the re-scan can now
  confirm or correct them.
- If the re-scan detects something that contradicts existing content, flag it
  as a `[CONFLICT]` comment for human review — do not resolve it silently.
- Show all proposed additions as a diff and prompt before writing.

---

### Sub-target: `architecture`

**Trigger:** new subsystems, services, or integrations have been added to the
codebase since the architecture docs were last written.

**Re-run discovery (in parallel):**
- `$FORGE_ROOT/init/discovery/discover-stack.md`
- `$FORGE_ROOT/init/discovery/discover-processes.md`
- `$FORGE_ROOT/init/discovery/discover-routing.md`

**Read existing docs:**
- `engineering/architecture/*.md`

**Merge into:**

| Discovery output | Target doc | Merge action |
|-----------------|-----------|-------------|
| New framework or runtime | `stack.md` | Append to technology inventory |
| New service or process | `processes.md` | Append new service section |
| New API route group | `routing.md` | Append route group |
| New deployment target | `deployment.md` | Append environment section |
| Any new sub-system with no existing doc | Create new sub-doc + link from `INDEX.md` |

---

### Sub-target: `business-domain`

**Trigger:** new ORM models, schema tables, or domain types have been added
to the codebase. `forge:health` will flag these as orphaned entities.

**Re-run discovery:**
- `$FORGE_ROOT/init/discovery/discover-database.md`

**Read existing doc:**
- `engineering/business-domain/entity-model.md`

**Merge into `entity-model.md`:**
- Entities present in discovery output but absent from the doc → append new
  entity sections with fields and relationships.
- New fields on an existing entity → add within the existing entity section,
  marked `[NEW]` for team review.
- Entities no longer found in the codebase → flag with `[NOT FOUND IN SCAN]`
  comment but do not remove (may be soft-deleted, feature-flagged, or in a
  migration).

---

### Sub-target: `stack-checklist`

**Trigger:** new libraries or frameworks have been adopted mid-project that
are not yet represented in review checklist items.

**Re-run discovery:**
- `$FORGE_ROOT/init/discovery/discover-stack.md`
- `$FORGE_ROOT/init/discovery/discover-testing.md`

**Read existing doc:**
- `engineering/stack-checklist.md`

**Merge into `stack-checklist.md`:**
- Libraries detected but not yet in the checklist → append new checklist items.
- Never remove or modify existing items (they encode accumulated review knowledge).

---

## Default (no argument)

Run all categories respecting dependencies — with maximum parallelism:

1. **Build brief** (once, synchronous):
   ```sh
   node "$FORGE_ROOT/tools/build-init-context.cjs" \
     --config .forge/config.json --personas .forge/personas \
     --templates .forge/templates \
     --kb "$(node "$FORGE_ROOT/tools/manage-config.cjs" get paths.engineering 2>/dev/null || echo engineering)" \
     --out .forge/init-context.md --json-out .forge/init-context.json
   ```

2. **Personas + Templates in parallel** (both need only KB, not each other):
   Spawn persona fan-out and template fan-out in a **SINGLE Agent tool message**
   (all persona subagents + all template subagents together). Wait for all to return.

3. **Skills + Workflows in parallel** (skills need personas; workflows need personas + templates — both now ready):
   Spawn skill fan-out and workflow fan-out (16 atomic) in a **SINGLE Agent tool message**.
   Wait for all to return.

4. **Orchestration + Commands in parallel** (both need workflows — now ready):
   Spawn orchestration and commands subagents in a **SINGLE Agent tool message**.
   Wait for both.

5. **workflows-js** (deterministic verbatim copy — no LLM, independent):
   Run the `workflows-js` category (full copy) as described above: copy every
   `.js` file from `$FORGE_ROOT/init/base-pack/workflows-js/` into
   `.claude/workflows/` and record manifest hashes. This step has no
   dependencies and can run alongside step 4.

This runs in a handful of serial steps instead of sequential per-category
passes, with all fan-outs parallelised within each step.

## Flag: `--enrich`

When `$ARGUMENTS` contains `--enrich`, run the enhancement workflow instead of regeneration.
This is the v1.0 replacement for the removed `/forge:enhance` command.

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

1. Check that `$FORGE_ROOT/meta/workflows/meta-enhance.md` exists. If absent:

   > △ meta-enhance.md not found — your installed Forge version may predate the enhancement agent. Run `/forge:update` to upgrade.

   Exit.

2. Pass `$ARGUMENTS` (minus the `--enrich` flag) through to the enhancement workflow so any phase flags (`--phase 1`, `--phase 2`, `--phase 3`, `--auto`) are forwarded:

   Read `$FORGE_ROOT/meta/workflows/meta-enhance.md` and follow it with the forwarded arguments.

   If no phase flag is provided with `--enrich`, the enhancement workflow defaults to `--phase 3` (drift detection).

3. Do NOT run any regeneration steps from the categories above — `--enrich` is a standalone mode.

---

## On error

If any step above fails unexpectedly, describe what went wrong and ask:

> "This looks like a Forge bug. Would you like to file a report to help improve it? Run `/forge:report-bug` — I'll pre-fill the report from this conversation."

---

## Post-regeneration persona pack

Rebuild the compact persona/skill reference pack at
`.forge/cache/persona-pack.json`. This is consumed by `meta-orchestrate` and
`meta-fix-bug` to inject persona references (not verbatim prose) into
subagent prompts when `FORGE_PROMPT_MODE=reference` (the default).

The pack compiles YAML frontmatter from `$FORGE_ROOT/meta/personas/meta-*.md`
and `$FORGE_ROOT/meta/skills/meta-*.md`. It is safe to rebuild on every
regenerate run (cost: ~50ms, 16 files).

```sh
node "$FORGE_ROOT/tools/build-persona-pack.cjs" \
  --out .forge/cache/persona-pack.json
```

- Exit 0: emit `〇 persona pack refreshed`
- Exit 1: surface the stderr message (it includes the offending file path
  for missing-frontmatter or malformed-YAML errors) and advise the user
  to file a bug if the error is unexpected.

## Post-regeneration context pack

Rebuild the architecture context pack at `.forge/cache/context-pack.md` and
`.forge/cache/context-pack.json`. This is injected into subagent prompts by
`meta-orchestrate` and `meta-fix-bug` to reduce per-phase architecture doc reads.

The pack summarises `engineering/architecture/*.md` (skips `*.draft.md`). If
the existing pack has `manual: true` in its frontmatter, the builder skips
and leaves it intact.

```sh
ENGINEERING=$(node "$FORGE_ROOT/tools/manage-config.cjs" get paths.engineering 2>/dev/null || echo engineering)
node "$FORGE_ROOT/tools/build-context-pack.cjs" \
  --arch-dir "$ENGINEERING/architecture" \
  --out-md .forge/cache/context-pack.md \
  --out-json .forge/cache/context-pack.json
```

- Exit 0: emit `〇 context pack refreshed`
- Exit 1: surface the stderr message — most likely the architecture directory
  does not exist yet (run after the knowledge-base category is populated).
  This is non-fatal for regenerate: emit a warning and continue.

## Post-regeneration verification

After all requested targets have been regenerated, verify structural completeness:

```sh
node "$FORGE_ROOT/tools/check-structure.cjs" --path .
```

- If exit 0: emit `〇 All expected generated files are present.`
- If exit 1: list the missing files by namespace and suggest running `/forge:rebuild <namespace>` for the affected category or categories.
