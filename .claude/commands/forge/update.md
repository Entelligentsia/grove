---
name: update
description: Check for Forge updates, review changes, install, and apply migrations — all in one command
---

# /forge:update

Single entry point for updating Forge. Checks GitHub for new versions, shows
what changed, guides you through the install, and applies migrations to this
project's generated artifacts.

## Locate plugin root

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

Detect install mode:

```
IS_CANARY = FORGE_ROOT does not contain "/.claude/plugins/"
```

- **Managed install** (`IS_CANARY = false`): plugin lives under the Claude Code
  plugins directory (either `/.claude/plugins/cache/` or
  `/.claude/plugins/marketplaces/`). Updated via the plugin manager.
- **Canary / source install** (`IS_CANARY = true`): FORGE_ROOT is outside the
  Claude Code plugins directory — a local source path (e.g. `/home/user/src/forge/forge`).
  The source is already at the correct version — there is nothing to install.
  Only migration steps apply.

---

## Model-alias auto-suppression pre-check

**Reusable sub-procedure — invoked from Steps 2A, 2B, and 4 after
aggregating `manual` items.**

When a migration chain includes the 0.6.13→0.7.0 step (or any step whose
`manual` list contains an item about custom model overrides), this pre-check
determines whether that manual item is a false positive for the current
project and removes it if so.

### Procedure

After the aggregation step has produced the `manual` list (and `breaking`
flag), but **before** displaying the breaking-change block or prompting for
confirmation:

1. **Identify model-override manual items.** Scan the aggregated `manual`
   list for any item whose text contains the substring
   `custom 'model' overrides in config.pipelines`. If none found, skip this
   entire sub-procedure — nothing to suppress.

2. **Read `.forge/config.json`.** If the file does not exist, or if it
   contains no `pipelines` key, or if `pipelines` is empty — there are no
   custom model overrides. Remove the matching manual item(s) and jump to
   step 4 below.

3. **Scan pipeline phases.** For every pipeline in `config.pipelines`, for
   every phase that has a `model` field, classify the value:
   - **Standard Forge aliases:** `sonnet`, `opus`, `haiku`
   - **Non-standard:** anything else (e.g. raw model IDs like
     `claude-3-opus`, `claude-sonnet-4-6`, or unknown aliases)

   If **all** `model` values across all pipelines are standard aliases or
   absent (no `model` field on the phase), the model-override manual item is
   a false positive. Remove it from `manual`.

   If **any** non-standard `model` value is found, the manual item is
   legitimate — keep it in `manual` and do not suppress the confirmation.

4. **Re-evaluate `breaking` flag.** After removing model-override items, if
   `manual` is now empty, set `breaking = false` for the current step's
   display/confirmation logic. A breaking-change section with zero manual
   items must not be shown.

### Result

The `manual` list and `breaking` flag are updated in-place. The calling step
then renders its summary and confirmation prompts using the filtered values —
no further changes to the step logic are needed.

---

## Progress Output Format

Open the run with the ember hero, then a subtitle:

```sh
node "$FORGE_ROOT/tools/banners.cjs" ember
node "$FORGE_ROOT/tools/banners.cjs" --subtitle "Updating Forge — checking remote, applying migrations, verifying state"
```

At the start of each step, emit a step header via `banners.cjs --phase`:

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase {N} 7 "{Step Name}" {bannerKey}
```

Step ↔ banner key map:

| Step | Name | Banner key |
|------|------|------------|
| 1 | Check for updates | `north` |
| 2A | Plugin update available | `rift` |
| 2B | Apply project migrations | `drift` |
| 3 | Verify installation | `lumen` |
| 4 | Apply migrations | `forge` |
| 5 | Pipeline audit | `oracle` |
| 6 | Record state | `drift` |
| 7 | Tomoshibi | `lumen` |

The `--phase` helper replaces the older `━━━ Step N/7 — <name> ━━━` emits
— do not emit a second em-dash banner after the helper. `banners.cjs`
strips ANSI in `NO_COLOR` / non-tty / `--plain` contexts.

---

## Resume Detection (FR-002)

Before Step 1, check whether a previous update left the project in a Pending
state. Read `.forge/update-check-cache.json` and check the `updateStatus` field.

If `updateStatus === "pending"`:

1. Print the pending state:
   ```
   △ Previous update is incomplete — pending migration(s): {pendingMigrations}
     Reason: {pendingReason}

     Run /forge:init --migrate to complete the pending migration, then re-run
     /forge:update to verify.
   ```

2. Do NOT proceed to Step 1 (remote version check) or Steps 2A, 2B, 3 — these
   were completed in the previous run. The project already has the correct
   plugin version installed; only the migration chain remains.

3. Exit. The user runs `/forge:init --migrate` to complete, then re-runs `/forge:update`.

If `updateStatus !== "pending"` (or field absent): proceed normally with Step 1.

---

## Step 1 — Check for updates

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 1 7 "Check for updates" north
```

Read `$FORGE_ROOT/.claude-plugin/plugin.json`. Extract `"version"` → `LOCAL_VERSION`.

Determine the distribution from `FORGE_ROOT` path — the cache path encodes the
marketplace name and is more reliable than reading fields from `plugin.json`:

| FORGE_ROOT contains | Distribution |
|---------------------|-------------|
| `/cache/skillforge/forge/` | `forge@skillforge` |
| `/marketplaces/skillforge/forge/` | `forge@skillforge` |
| anything else | `forge@forge` / canary |

For **both** distributions, resolve `UPDATE_URL` and `MIGRATIONS_URL` from the
installed `plugin.json` — each distribution branch ships its own correct URLs:

```
UPDATE_URL     = plugin.json → updateUrl,     fallback: https://raw.githubusercontent.com/Entelligentsia/forge/main/forge/.claude-plugin/plugin.json
MIGRATIONS_URL = plugin.json → migrationsUrl, fallback: https://raw.githubusercontent.com/Entelligentsia/forge/main/forge/migrations.json
```

**Do NOT hardcode per-distribution URLs** — the installed `plugin.json` is the
authoritative source. Hardcoding breaks when distribution hosting moves.

Set `UPDATE_URL`, `MIGRATIONS_URL`, and `DISTRIBUTION` accordingly before fetching.

Also read `distribution` from `.forge/update-check-cache.json` → `PRIOR_DISTRIBUTION`.
If the file doesn't exist or the field is absent, set `PRIOR_DISTRIBUTION = DISTRIBUTION`.

Fetch the **remote** plugin manifest to get the latest available version.
Use the WebFetch tool (preferred) or `curl` via Bash:

```
URL: {UPDATE_URL}
```

Parse the response JSON and extract the `version` field → `REMOTE_VERSION`.

If the fetch fails (network error, timeout), warn the user:
> Could not reach GitHub to check for updates. Proceeding with local version only.

Then skip to **Step 3** (apply pending migrations if any).

### Determine update status

Read the migration baseline from the project-scoped update-check cache:

```
CACHE_FILE = .forge/update-check-cache.json
```

This file is project-scoped — each project maintains its own migration state.
Read it if it exists.

**Baseline derivation formula (evaluated in order, stop at first defined value):**

```
baseline = migratedFrom ?? localVersion ?? LOCAL_VERSION
```

- `migratedFrom` — written by Step 4 of the last successful update run. It represents
  "the version from which the last successful migration chain was applied to this project"
  (i.e. the baseline of the prior migration, not the version the project is currently at).
  This field was introduced in v0.30.0; older cache files may not have it.
- `localVersion` — the version recorded when the cache was last written. Used when
  `migratedFrom` is absent (cache file predates v0.30.0 or was written before the first
  migration ran).
- `LOCAL_VERSION` — the installed plugin version. Last resort when the cache file does not
  exist at all (fresh project, or cache deleted).

> **Semantics note:** `migratedFrom` may be *lower* than `localVersion` — it is the
> starting point of the last migration chain, not the end point. Example: a project that
> migrated from 0.28.0 to 0.32.0 has `migratedFrom: "0.28.0"` and `localVersion: "0.32.0"`.
> The next update run uses `0.28.0` as the baseline to walk forward from. This is correct —
> any migrations between 0.28.0 and 0.32.0 that were applied should be idempotent, and
> any new steps after 0.32.0 will be picked up.

> **Legacy fallback:** If `.forge/update-check-cache.json` does not exist but a
> plugin-level cache does (`${CLAUDE_PLUGIN_DATA}/forge-plugin-data/update-check-cache.json`
> or `/tmp/forge-plugin-data/update-check-cache.json`), read `migratedFrom` from
> there as a one-time migration. Step 6 will write the project-scoped file going forward.

The user can also pass `--from <version>` as an argument to set the baseline
explicitly — this overrides any cached value.

### Baseline derivation — worked examples

- **Scenario A:** Cache contains `{ "migratedFrom": "0.28.0", "localVersion": "0.32.0" }` → baseline is `0.28.0` (`migratedFrom` takes precedence).
- **Scenario B:** Cache contains `{ "localVersion": "0.29.0" }` (no `migratedFrom` — pre-v0.30.0 cache) → baseline is `0.29.0`.
- **Scenario C:** Cache file absent → baseline is `LOCAL_VERSION` (the currently installed version).

Now evaluate — **stop at the first matching row and follow only that row's action**:

| # | Condition | Action |
|---|-----------|--------|
| 1 | `REMOTE_VERSION` == `LOCAL_VERSION` and `LOCAL_VERSION` == baseline | Print "Forge {LOCAL_VERSION} — up to date. No pending migrations." Then execute **Step 4 config refresh** (paths.forgeRef, backfill) and proceed to **Step 5**. |
| 2 | `REMOTE_VERSION` == `LOCAL_VERSION` and `LOCAL_VERSION` != baseline | Jump to **Step 2B** (project migration — no install needed). |
| 3 | `IS_CANARY` is true | Jump to **Step 2B** (canary — no install needed). |
| 4 | `LOCAL_VERSION` > `REMOTE_VERSION` | Print "Local version ({LOCAL_VERSION}) is ahead of the release channel ({REMOTE_VERSION}). No install needed — applying any pending project migrations." then jump to **Step 2B**. |
| 5 | `REMOTE_VERSION` != `LOCAL_VERSION` | Proceed to **Step 2A** (plugin update available). |

> **Row 4 worked example:** If `LOCAL_VERSION` is `0.35.0` and `REMOTE_VERSION` is
> `0.32.0` (user built from source or is on a forward canary), row 4 triggers — project
> migrations from baseline forward are applied, but no install prompt is shown. This
> handles cases where `IS_CANARY` was not detected (e.g. a managed install whose
> `plugin.json` version was manually bumped ahead of the release branch).

**Do NOT show an install prompt for rows 1, 2, 3, or 4. Install prompts only appear in Step 2A.**

---

## Step 2A — Plugin update available

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 2 7 "Plugin update available" rift
```

> **Only reached when `REMOTE_VERSION` != `LOCAL_VERSION` (row 4 above).**

Fetch the **remote** migrations manifest from GitHub:

```
URL: {MIGRATIONS_URL}
```

Parse the response JSON. Walk the migration chain from `LOCAL_VERSION` forward
to `REMOTE_VERSION`. Aggregate across all steps:
- Union of all `regenerate` targets, applying the dominance rule: for each
  category (`workflows`, `knowledge-base`, `commands`, `tools`), if any step lists a
  bare category name (e.g. `"workflows"`), that category is flagged for full
  rebuild. If all steps for a category use sub-targets (e.g.
  `"workflows:plan_task"`), collect the union of those sub-targets
  (deduplicated, order preserved).
- Concatenated `notes` (one line per step)
- `breaking: true` if any step is breaking
- Union of all `manual` items

**Run the Model-alias auto-suppression pre-check** (see section above)
on the aggregated `manual` list and `breaking` flag before displaying
the summary below.

Present the update summary:

```
## Forge Update Available

{LOCAL_VERSION} → {REMOTE_VERSION}

### What's new
{for each step in path:}
  • {version}: {notes}

### After install, Forge will regenerate
  {for each category in aggregated result:}
  {if full rebuild:}
  • {category}: (full rebuild)
  {else:}
  • {category}: {sub-target1}, {sub-target2}, ...

{if breaking:}
### △ Breaking changes — manual steps required
  {for each item in manual:}
  • {item}

### How to proceed
  [1] Install now — I'll guide you through it
  [2] Skip for now
```

If no migration path can be constructed, show available notes and recommend
`/forge:rebuild workflows`.

Ask the user to choose. If they choose **[2]**, exit.

If they choose **[1]**, proceed to **Guided install** below.

### Guided install

**If `IS_CANARY` is true** (safety net — should have been caught by row 3):

Print:
```
Canary install detected — FORGE_ROOT is a local source directory, not the
plugin cache. There is nothing to install via the plugin manager.

Your source is already at {LOCAL_VERSION}. Proceeding directly to migrations.
```

Jump to **Step 4**.

**If `IS_CANARY` is false (marketplace install):**

Print:
```
To install the update:

  1. Run /plugin to open the plugin manager
  2. Find Forge in your installed plugins and update it

Tell me when the install is done.
```

Wait for the user to confirm the install completed.

---

## Step 2B — Project migration pending (plugin already current)

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 2 7 "Apply project migrations" drift
```

> **Only reached from rows 2 or 3 — the plugin is already at the right version.**
> **Do NOT show an install prompt here. There is nothing to install.**

Read `$FORGE_ROOT/migrations.json` (local).

Walk the migration chain from `baseline` forward to `LOCAL_VERSION`. Aggregate:
- Union of all `regenerate` targets, applying the dominance rule: for each
  category (`workflows`, `knowledge-base`, `commands`, `tools`), if any step lists a
  bare category name (e.g. `"workflows"`), that category is flagged for full
  rebuild. If all steps for a category use sub-targets (e.g.
  `"workflows:plan_task"`), collect the union of those sub-targets
  (deduplicated, order preserved).
- Concatenated `notes`
- `breaking: true` if any step is breaking
- Union of all `manual` items

**Run the Model-alias auto-suppression pre-check** (see section above)
on the aggregated `manual` list and `breaking` flag before printing
the summary below.

Print:

```
## Forge {LOCAL_VERSION} — Plugin up to date

Your project was last migrated at {baseline}. The following changes need
to be applied to this project's generated files:

### Changes since {baseline}
{for each step in path:}
  • {version}: {notes}

### Will regenerate
  {for each category in aggregated result:}
  {if full rebuild:}
  • {category}: (full rebuild)
  {else:}
  • {category}: {sub-target1}, {sub-target2}, ...

{if breaking:}
### △ Breaking changes — complete these steps first:
  {for each item in manual:}
  • {item}

Apply migrations now? [Y/n]
```

If the user declines, exit without changes.
If `breaking: true`, confirm they have completed the manual steps first.

Then jump to **Step 4** to execute the regeneration.

---

## Step 3 — Verify installation

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 3 7 "Verify installation" lumen
```

After the user confirms the install:

Re-read the local plugin version:
```
$FORGE_ROOT/.claude-plugin/plugin.json   →   extract "version"   →   NEW_LOCAL_VERSION
```

| Condition | Action |
|-----------|--------|
| `NEW_LOCAL_VERSION` == `REMOTE_VERSION` | Print "〇 Forge {NEW_LOCAL_VERSION} installed successfully." and proceed to **Step 4**. |
| `NEW_LOCAL_VERSION` == `LOCAL_VERSION` (unchanged) | Warn: "The plugin version hasn't changed ({LOCAL_VERSION}). The install may not have completed. Would you like to try again or continue anyway?" If user wants to continue, proceed to **Step 4** using available version. |
| `NEW_LOCAL_VERSION` is different but not `REMOTE_VERSION` | Print "Installed Forge {NEW_LOCAL_VERSION} (expected {REMOTE_VERSION}). Proceeding with {NEW_LOCAL_VERSION}." and continue to **Step 4**. |

Update `LOCAL_VERSION` to `NEW_LOCAL_VERSION` for subsequent steps.

### Re-derive FORGE_ROOT

After verifying the new version, re-derive `FORGE_ROOT`. A managed plugin
install changes the cache path (e.g. `…/cache/forge/forge/0.9.6/` →
`…/cache/forge/forge/0.9.9/`), so the `FORGE_ROOT` captured at the top of
this command is stale.

```sh
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

If the re-derived `FORGE_ROOT` differs from the original value, print:
> 〇 FORGE_ROOT updated: {old} → {new}

If `IS_CANARY` is true, `FORGE_ROOT` never changes (it is a local source
path) — skip the re-derivation and keep the original value.

---

## Step 4 — Apply migrations

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 4 7 "Apply migrations" forge
```

> **Config refresh always runs.** The Step 4 header section (forgeRef, backfill)
> executes regardless of whether migrations are pending — even Row 1
> ("up to date") proceeds through this section before skipping to Step 5. The
> "skip to Step 5" directive skips only the migration chain walk and regeneration.
> Missing config fields can accumulate across version boundaries; backfill ensures
> the config stays structurally complete after every `/forge:update` invocation.

**Write `paths.forgeRef` (FR-010):** Write the installed plugin version
as `paths.forgeRef` to config. This makes the config portable across machines —
`forgeRef` is a version string rather than an absolute path, and is used by
`forge-preflight.cjs` to resolve the plugin root via cache lookup:

```sh
LOCAL_VERSION=$(node -e "console.log(require('$FORGE_ROOT/.claude-plugin/plugin.json').version)")
node "$FORGE_ROOT/tools/manage-config.cjs" set paths.forgeRef "$LOCAL_VERSION"
```

**Backfill missing config fields:** Config schema evolves across versions — new
required or recommended fields may have been added since the project was last
initialized. `manage-config backfill` reads the config schema, compares it
against the current `.forge/config.json`, and writes defaults for any missing
fields with schema-defined defaults. It also stamps the top-level `version`
field from the bundled plugin version. Run after setting forgeRef:

```sh
node "$FORGE_ROOT/tools/manage-config.cjs" backfill --forge-root "$FORGE_ROOT"
```

If the project was recently initialized and all fields are already present, the
command prints `〇 No missing fields to backfill.` and exits cleanly —
no changes, no side effects.

**Fix `paths.commands` prefix (v1.0.2+):** Pre-v1.0.2 projects may have
`paths.commands` set to `.claude/commands/forge` regardless of the project
prefix. Correct it to use the lowercased project prefix:

```sh
PREFIX=$(node -e "try{console.log(require('./.forge/config.json').project.prefix.toLowerCase())}catch{console.log('forge')}")
CURRENT_COMMANDS=$(node "$FORGE_ROOT/tools/manage-config.cjs" get paths.commands 2>/dev/null || echo "")
EXPECTED_COMMANDS=".claude/commands/$PREFIX"
if [ "$CURRENT_COMMANDS" != "$EXPECTED_COMMANDS" ]; then
  node "$FORGE_ROOT/tools/manage-config.cjs" set paths.commands "$EXPECTED_COMMANDS"
fi
```

Determine the baseline version:
- Use `migratedFrom` from `CACHE_FILE` (set in Step 1)
- Or the `--from <version>` argument if provided
- Or the pre-install `LOCAL_VERSION` (before Step 3 updated it)

If `LOCAL_VERSION` equals baseline, there are no migrations to apply — skip
to **Step 5**.

Read `$FORGE_ROOT/migrations.json` (local — now updated after install).

Before walking the migration chain, check for a cross-distribution downgrade:
if `PRIOR_DISTRIBUTION` ≠ `DISTRIBUTION` and baseline appears higher than
`LOCAL_VERSION` (e.g. baseline is `1.1.0` and `LOCAL_VERSION` is `1.0.5`):

> The migration baseline ({baseline}) was set on **{PRIOR_DISTRIBUTION}** and is
> higher than the current plugin version ({LOCAL_VERSION} on {DISTRIBUTION}).
> The {PRIOR_DISTRIBUTION} migration chain does not exist in {DISTRIBUTION}'s
> migrations.json — walking it would fail.
>
> Reset migration baseline to {LOCAL_VERSION} and regenerate workflows to match
> the current installed version? This is safe — it re-generates files from the
> installed plugin, discarding the unreachable canary state. [Y/n]

If yes: set `baseline = LOCAL_VERSION`. If baseline now equals `LOCAL_VERSION`,
there are no migrations to apply — skip directly to **Step 6**.

If no: exit without changes.

Walk the migration chain from baseline forward to `LOCAL_VERSION`:
- Each entry key is a `from` version; its `version` field is the `to` version.
- Collect the ordered list of migration steps that bridge baseline → current.
- If no path exists, warn:
  > No migration path found from {baseline} to {LOCAL_VERSION}. Running
  > `/forge:rebuild workflows` is recommended.
  Then exit.

Aggregate across all steps in the path, applying the dominance rule:
- For each category (`workflows`, `knowledge-base`, `commands`, `tools`):
  - If ANY step has a bare entry for this category → full rebuild for that category.
  - Otherwise → union of all sub-targets across all steps (deduplicated, order preserved).
- Concatenated `notes` (one line per step)
- `breaking: true` if any step is breaking
- Union of all `manual` items

**Run the Model-alias auto-suppression pre-check** (see section above)
on the aggregated `manual` list and `breaking` flag before displaying
the confirmation prompt below.

### Regeneration order

Execute regeneration targets in this order:

| Order | Target | Can run after | Special handling |
|-------|--------|---------------|-----------------|
| 0 | `hooks` | — (N/A) | **No-op for project copies.** Hooks ship with the plugin — they are already updated at the time of install. Emit: `〇 hooks — updated via plugin install (no project copy to regenerate)` |
| 1 | `tools` | — (independent) | — |
| 2 | `workflows` | — (independent) | — |
| 3 | `templates` | — (independent) | — |
| 4 | `personas` | — (independent) | — |
| 5 | `commands` | Must run after `workflows` | — |
| 6 | `knowledge-base` sub-targets | — (independent) | — |
| 7 | `schemas` | — (independent) | **Run schema refresh inline** (see Schema Refresh below). Emit: `〇 Refreshing schemas…` |
| 8 | `workflows-js` | — (independent) | Deterministic verbatim copy of `.claude/workflows/*.js` from base-pack via `/forge:rebuild workflows-js` (no LLM, no placeholder substitution). |

> **Known special targets — note for migration authors:** `hooks` and `schemas` are
> special-cased here. Future `migrations.json` entries should only use recognised
> target names; using unknown bare-category targets will produce a warning and be
> skipped. The recognised targets are: `hooks`, `tools`, `workflows`, `templates`,
> `personas`, `commands`, `knowledge-base`, `skills`, `schemas`, `workflows-js`.
> `workflows-js` accepts granular sub-targets (e.g. `workflows-js:wfl-run-task`).

`commands` depends on `workflows` because command wrappers reference workflow
filenames. All other targets are independent and could run in parallel, but
are executed sequentially here to keep the output readable.

Only execute targets that appear in the aggregated result — skip absent ones.

### Subagent probe (FR-009)

Before offering the regeneration confirmation prompt, probe whether subagent
dispatch will succeed in this session.

**Probe step:** Attempt to invoke a lightweight read-only command via the Skill
tool: call `skill: "forge:health"` with no arguments. This is a read-only
operation that checks knowledge base currency — it cannot modify any state and
is safe to invoke as a probe.

**Success determination:** If the Skill tool call returns without error (exit 0
or a normal response), the probe succeeds. Note that subagent dispatch is
available.

**Failure determination:** If the Skill tool call is unavailable (tool not
found), returns a permission error, or returns any unexpected error, the probe
fails. Note that subagent dispatch is not available in this session.

If the probe fails:
- Warn explicitly: "Subagent dispatch is not available in this session.
  Regeneration will run inline (in the current session)."
- If the A/B/C regeneration choice is offered (Step 2A), mark option A
  (regenerate now) as "will fall back to inline execution" or remove it
  entirely if option B (defer to fresh session) is safer.

**Mid-orchestration failure handling:** If a regeneration sub-step dispatches
via Agent tool and the agent fails mid-execution:
- Do NOT silently fall back to inline execution.
- Re-prompt the user with the failure details and three explicit options:
  - **(a) Retry:** Re-attempt the failed step using the Agent tool.
  - **(b) Defer:** Skip remaining steps. Mark the update as Pending with a
    reason noting the subagent failure. The user can complete remaining steps
    in a fresh session.
  - **(c) Skip:** Skip the failed step (with warning that some generated
    artifacts may be missing), continue with remaining steps inline.

**Success report flagging:** In the Step 6 summary, include a line:
`Subagent isolation: {used | bypassed (inline)}` to make it clear which mode
was used.

### Confirm and regenerate

Print a migration summary:

```
## Applying Migrations: {baseline} → {LOCAL_VERSION}

Changes:
  {notes from each step, one per line}

Regeneration targets:
  {for each category in aggregated result:}
  {if full rebuild:}
  • {category}: (full rebuild)
  {else:}
  • {category}: {sub-target1}, {sub-target2}, ...

{if breaking:}
△ Breaking changes — complete these manual steps first:
  {manual items}

Proceed? [Y/n]
```

If the user declines, exit without modifying anything.
If `breaking: true`, require the user to confirm they have completed the manual
steps before proceeding.

For each category in the aggregated result, invoke `/forge:rebuild` by
reading and following `$FORGE_ROOT/commands/regenerate.md`:
- If flagged for full rebuild: invoke `/forge:rebuild <category>`
- If sub-targets collected: invoke `/forge:rebuild <category> <sub-target>`
  for each sub-target in order

**Category-to-command mapping:** most categories are handled by
`/forge:rebuild`, but the `tools` and `schemas` categories are special.

When `tools` appears in the aggregated result, invoke `/forge:rebuild tools`
to re-copy the current plugin's tools closure into `.forge/tools/`. This is
the actual re-vendor step — do NOT run schema refresh inline instead.

When `schemas` appears in the aggregated result, run the schema refresh inline
(see **Schema Refresh** section below). Do NOT delegate to the removed `/forge:update-tools` command.

### Schema Refresh

When the migration chain includes a `schemas` target, refresh schemas inline:

```sh
mkdir -p .forge/schemas
cp "$FORGE_ROOT/schemas/"*.schema.json .forge/schemas/
for f in .forge/schemas/*.schema.json; do
  node "$FORGE_ROOT/tools/generation-manifest.cjs" record "$f"
done
node "$FORGE_ROOT/tools/validate-store.cjs" --dry-run
```

Emit `〇 Schemas updated and store validation passed.` on success.
Emit `× Validation failed — {output}` on non-zero exit from `validate-store.cjs`.

Run non-knowledge-base targets first (workflows, templates, commands, tools),
then knowledge-base sub-targets if present.

**Sub-target filename resolution:** a sub-target like `architect_sprint_plan`
maps to the file `.forge/workflows/architect_sprint_plan.md` — append `.md`
to the sub-target name as written. Do NOT strip any prefix or suffix.

### Post-migration structure check

After all regeneration targets complete, run:
```sh
node "$FORGE_ROOT/tools/check-structure.cjs" --path .
```

If exit 0 (all present):
> 〇 All expected generated files are present.

If exit 1 (gaps remain):
> △ Structure check: N file(s) still missing after migration:
>   (list missing files)
>
> This may indicate a failed regeneration step. Re-run `/forge:rebuild <namespace>`
> for each affected namespace, or `/forge:rebuild` to rebuild all targets.
> Note: skills entries require an explicit `/forge:rebuild skills` — they are not
> included in the default regenerate run.

Do NOT block migration success on gaps — surface them as a warning only. The user
is already informed of failed regeneration steps by the Iron Laws above; this check
is an additional safety net.

### Refresh calibrationBaseline

After the structure check, if at least one regeneration target was applied (the
aggregated `regenerate` list was non-empty), refresh `calibrationBaseline` in
`.forge/config.json`. This keeps the calibration baseline in sync with the newly
materialized artifacts so `/forge:health --fix` does not report false drift.

Skip this step if no regeneration targets were applied (e.g. all targets were absent
from the migration path, or Step 4 was entered with `baseline == LOCAL_VERSION`).

1. Read `$FORGE_ROOT/.claude-plugin/plugin.json` → `version`.
2. Resolve `KB_PATH`:
   ```sh
   KB_PATH=$(node "$FORGE_ROOT/tools/manage-config.cjs" get paths.engineering 2>/dev/null || echo "engineering")
   ```
3. Hash `{KB_PATH}/MASTER_INDEX.md` (strip blank lines + `<!--` lines, SHA-256):
   ```sh
   node -e "const crypto=require('crypto'),fs=require('fs'); const lines=fs.readFileSync('${KB_PATH}/MASTER_INDEX.md','utf8').split('\n').filter(l=>l.trim()&&!l.trim().startsWith('<!--')); console.log(crypto.createHash('sha256').update(lines.join('\n')).digest('hex'))"
   ```
4. List done sprint IDs from `.forge/store/sprints/` using union-merge
   (FR-003: cumulative provenance — `sprintsCovered` must never shrink):
   ```sh
   node -e "const fs=require('fs'),p='.forge/store/sprints'; try{const files=fs.readdirSync(p).filter(f=>f.endsWith('.json')); const done=files.map(f=>JSON.parse(fs.readFileSync(p+'/'+f,'utf8'))).filter(s=>['done','retrospective-done'].includes(s.status)).map(s=>s.sprintId); console.log(JSON.stringify(done));}catch(e){console.log('[]')}"
   ```
5. Today's ISO date: `date -u +"%Y-%m-%d"`
6. **Union-merge `sprintsCovered`** (FR-003): read the existing
   `calibrationBaseline.sprintsCovered` from config, compute the union with
   the current done sprint IDs:
   ```sh
   node -e "
   const fs = require('fs');
   const cfg = JSON.parse(fs.readFileSync('.forge/config.json','utf8'));
   const existing = new Set(cfg.calibrationBaseline ? cfg.calibrationBaseline.sprintsCovered || [] : []);
   const current = new Set(<computed done sprint IDs from step 4>);
   const merged = [...new Set([...existing, ...current])];
   const cfg2 = JSON.parse(fs.readFileSync('.forge/config.json','utf8'));
   cfg2.calibrationBaseline = {
     lastCalibrated: '<date>',
     version: '<ver>',
     masterIndexHash: '<hash>',
     sprintsCovered: merged
   };
   fs.writeFileSync('.forge/config.json', JSON.stringify(cfg2, null, 2) + '\n');
   "
   ```
   If the existing `sprintsCovered` array was shrunk in a prior version (i.e. the
   existing array is shorter than a heuristic would suggest), perform a **backfill
   scan** before the union-merge. Broaden the scan to include sprints with these
   terminal and historical status values: `done`, `retrospective-done`, `completed`,
   `partially-completed`, and `abandoned`. These statuses cover: (a) the current
   canonical terminal statuses (`done`, `retrospective-done`); (b) `completed`,
   which was the pre-v0.40 status for what became `retrospective-done`;
   (c) `partially-completed` and `abandoned`, which represent sprints whose outcomes
   may still have been included in a prior calibration baseline. Present the
   recovered array for user confirmation before writing.

Emit: `〇 calibrationBaseline refreshed — version <ver>, hash <first-8-chars-of-hash>...`

### Post-migration pack rebuild

After the calibrationBaseline refresh, if the aggregated regeneration targets
include any of `workflows`, `personas`, or `skills` (bare category or sub-target),
rebuild the persona and context packs so they reflect the newly regenerated artifacts.

If no `workflows`, `personas`, or `skills` targets were applied (e.g. only `tools`,
`schemas`, `commands`, or `knowledge-base` were in the migration path), skip this
step entirely — these categories do not affect the packs.

**1. Rebuild persona pack:**

```sh
node "$FORGE_ROOT/tools/build-persona-pack.cjs" --out .forge/cache/persona-pack.json
```

- Exit 0: emit `〇 persona pack refreshed`
- Exit 1: surface error, warn user (non-fatal — pack will refresh on next full regenerate)

**2. Rebuild context pack:**

```sh
ENGINEERING=$(node "$FORGE_ROOT/tools/manage-config.cjs" get paths.engineering 2>/dev/null || echo engineering)
node "$FORGE_ROOT/tools/build-context-pack.cjs" \
  --arch-dir "$ENGINEERING/architecture" \
  --out-md .forge/cache/context-pack.md \
  --out-json .forge/cache/context-pack.json
```

- Exit 0: emit `〇 context pack refreshed`
- Exit 1: warn (non-fatal — architecture directory may not yet exist for new projects)

Full Step 4 post-migration sequence: regenerate → structure check → calibrationBaseline refresh → pack rebuild.

### Migration completion gate (FR-002)

After the post-migration pack rebuild, evaluate the migration chain against the
ADR-S14-01 decision criteria to determine the update outcome:

**Low-risk classification** (all of these must be true for the step):
- `breaking: false` or `breaking` field absent
- No `manual` items (or all `manual` items resolved by model-alias auto-suppression)
- No net-new `.forge/` files requiring user-provided data
- No changes to `calibrationBaseline` semantics

Concretely, a low-risk step is one where the only actions are: schema-only
changes (inline schema refresh for schemas), file-copy from base-pack
(templates, workflows, personas), or config-key additions with deterministic
defaults.

**User-affecting classification** (any of these triggers Pending for the
entire chain):
- `breaking: true`
- `manual` items present after auto-suppression
- Net-new `.forge/` files requiring user-provided data (e.g.
  `project-context.json`)
- Changes to `calibrationBaseline` semantics (e.g. `sprintsCovered` reduction)

#### Auto-invoke deterministic path (ADR-S14-01 D6)

If all migration steps are low-risk, auto-invoke the deterministic migration
path inline within Step 4, after regeneration targets and `check-structure.cjs`
pass:

1. Check whether the migration chain includes any net-new `.forge/` files not
   covered by the `regenerate` targets. If none, skip to the normal
   post-migration sequence (calibrationBaseline refresh, pack rebuild) — the
   auto-invoke is a no-op.

2. If net-new files exist and the migration is low-risk, execute the
   deterministic migration path:
   - **Structural migration (Step 0 of `migrate.md`):** run
     `check-structure.cjs` to detect whether `structure-versions.json` is
     absent (indicating a pre-T05 install). If absent, read and follow
     `$FORGE_ROOT/meta/workflows/meta-migrate.md` Phase 0 through Phase 1
     (extraction) and Phase 2 (confirmation), then Phase 3 (write) — but ONLY
     the deterministic file-copy operations (archive, substitute, register).
     **Do NOT execute the interactive interview (Phase 1 extraction that
     requires user input, or Phase 2 confirmation).** The auto-invoke path
     only performs operations that can be completed without any user prompts.
   - **Deterministic file copies from base-pack:** if
     `structure-versions.json` already exists (post-T05 install), the
     deterministic path is simply running any remaining `/forge:rebuild`
     targets that were not already applied in the main Step 4 regeneration
     sequence.

3. If the deterministic migration path completes successfully, proceed to the
   normal Step 4 post-migration sequence (calibrationBaseline refresh, pack
   rebuild).

4. **Fallback to Pending:** If the deterministic migration path fails at any
   point (e.g. `check-structure.cjs` reports missing files after auto-invoke,
   a tool exits non-zero, or an unexpected requirement for user input is
   detected), immediately fall back to the Pending state:
   - Do NOT bump `calibrationBaseline.version`.
   - Write `updateStatus: "pending"`, `pendingReason: "Auto-invoke failed:
     {error description}"`, and `pendingMigrations: [list]` to
     `.forge/update-check-cache.json`.
   - Print: "Update Pending — auto-invoke failed. Run `/forge:init --migrate` to
     complete" with the failure details.
   - Exit without proceeding to Steps 5-7.

**Key invariant (from ADR-S14-01 D6):** The auto-invoke never prompts the user
for migration decisions. If any migration step needs user input, the entire
chain is classified as user-affecting and enters Pending.

#### User-affecting path — Pending state

If any step is user-affecting (and the chain was NOT auto-invoked), enter the
**Pending** state:
- Do NOT bump `calibrationBaseline.version`.
- Write `updateStatus: "pending"`, `pendingReason`, and `pendingMigrations` to
  `.forge/update-check-cache.json`.
- Print: "Update Pending — run `/forge:init --migrate` to complete" with the list of
  pending migrations and next steps.
- Exit without proceeding to Steps 5-7.

#### Update Complete path

On the "Update Complete" path (all migrations applied,
`check-structure.cjs` passes): set `updateStatus: "complete"`,
`pendingReason: null`, `pendingMigrations: []` in the cache file.

### Iron Laws for Step 4

- YOU MUST NOT call `generation-manifest.cjs record` directly for migration targets.
  The regenerate command records hashes after writing — calling record on a file that
  has not been regenerated yet produces a stale hash and silently corrupts the manifest.
- YOU MUST NOT inline the regeneration. Read `$FORGE_ROOT/commands/regenerate.md`
  and follow it as the authoritative procedure for every target.
- YOU MUST NOT declare success if any regeneration step errors. Surface the error
  to the user and stop. Do not continue to Step 5 with a failed regeneration.

---

## Step 5 — Pipeline and configuration audit

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 5 7 "Pipeline audit" oracle
```

Runs on every update. Collects all findings first, then presents a single
consolidated prompt. **Nothing is written without the user saying yes.**

### 5a — Locate tools

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

All tools are invoked directly from the plugin:
- `node "$FORGE_ROOT/tools/manage-config.cjs"`
- `node "$FORGE_ROOT/tools/generation-manifest.cjs"`

If `.forge/config.json` does not exist, skip this step and proceed to **Step 6**.

### 5b-collect — Run all sub-checks silently

Run each sub-check without prompting. Accumulate findings into an `AUDIT_ITEMS`
list. Each item has:

| Field | Description |
|-------|-------------|
| `type` | `delete-command`, `delete-workflow`, `update-pipeline-cmd`, `legacy-model-field`, `add-workflow-field`, `add-persona-symbol`, `add-paths-key`, `missing-command-file`, `add-gitignore-entry` |
| `label` | Human-readable one-line description |
| `action` | What will be done if approved |
| `path` | File or pipeline affected |
| `modified` | `true` if user edits detected (from manifest check); `false` otherwise |
| `required` | `true` for items requiring confirmation; `false` for auto-applied or advisory items |

**Item classification:**

| Type | `required` | `modified` can be true? | Notes |
|------|-----------|------------------------|-------|
| `delete-command` | true | yes (△) | Retired command file |
| `delete-workflow` | true | yes (△) | Retired workflow file |
| `update-pipeline-cmd` | true | no | Retired command name in pipeline config |
| `legacy-model-field` | false | no | Auto-migrated by regeneration |
| `add-workflow-field` | true | no | Missing workflow field in pipeline phase |
| `add-persona-symbol` | false | no | Optional decoration |
| `add-paths-key` | true | no | Missing config key |
| `missing-command-file` | false | no | Advisory only |
| `add-gitignore-entry` | true | no | Transient store path not gitignored |

Items where `modified: true` must be flagged with `△` in the label.

**Sub-checks to run silently** (logic unchanged from previous Step 5, just no
prompts — accumulate items instead):

**5b-pre — Retired generated files.** Check both `.claude/commands/` and
`.forge/workflows/` for retired filenames.

Old command files (exact match only):
- `engineer.md` → retired in favour of `plan.md`
- `supervisor.md` → retired in favour of `review-plan.md` / `review-code.md`

Do NOT match partial names, prefixes, or variants — those are custom commands.

For each found command file, check manifest status:
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" check .claude/commands/{old-name}.md
```

- Pristine/untracked → add item: `type: delete-command`, `modified: false`,
  `path: .claude/commands/{old-name}.md`,
  `label: ".claude/commands/{old-name}.md — retired name, safe to remove"`,
  `action: "Delete old file"`
- Modified (exit 1) → add item: `type: delete-command`, `modified: true`,
  `path: .claude/commands/{old-name}.md`,
  `label: "△ .claude/commands/{old-name}.md — retired name, user edits detected. Merge into {new-name}.md before deleting"`,
  `action: "Delete old file after merge"`
- Manifest tool absent → add item: `type: delete-command`, `modified: false`,
  `path: .claude/commands/{old-name}.md`,
  `label: ".claude/commands/{old-name}.md — retired name, cannot verify modifications"`,
  `action: "Delete old file"`

Old workflow files:

```
engineer_plan_task.md        → renamed to plan_task.md
engineer_implement_plan.md   → renamed to implement_plan.md
engineer_commit_task.md      → renamed to commit_task.md
engineer_update_plan.md      → renamed to update_plan.md
engineer_update_implementation.md → renamed to update_implementation.md
engineer_fix_bug.md          → renamed to fix_bug.md
supervisor_review_plan.md    → renamed to review_plan.md
supervisor_review_implementation.md → renamed to review_code.md
orchestrate_task.md          → retired (v1.2.0); orchestration runs through .claude/workflows/wfl-run-task.js
run_sprint.md                → retired (v1.2.0); orchestration runs through .claude/workflows/wfl-run-sprint.js
fix_bug.md                   → retired (v1.2.0); orchestration runs through .claude/workflows/wfl-fix-bug.js
```

> **Note:** `orchestrate_task.md` / `run_sprint.md` / `fix_bug.md` are LLM
> orchestration prose retired in v1.2.0 — they are no longer generated, and the
> deterministic JS drivers in `.claude/workflows/wfl-*.js` are the only
> orchestration truth. They are listed here so `/forge:update` removes the
> orphaned files from `.forge/workflows/` (the `/forge:rebuild` regeneration
> only clears manifest entries, it does not delete files on disk).

For each found workflow file, check manifest status:
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" check .forge/workflows/{old-name}.md
```

- Pristine/untracked → add item: `type: delete-workflow`, `modified: false`,
  `path: .forge/workflows/{old-name}.md`,
  `label: ".forge/workflows/{old-name}.md — retired workflow, safe to remove"`,
  `action: "Delete old workflow"`
- Modified (exit 1) → add item: `type: delete-workflow`, `modified: true`,
  `path: .forge/workflows/{old-name}.md`,
  `label: "△ .forge/workflows/{old-name}.md — retired workflow, user edits detected"`,
  `action: "Delete old workflow after merge"`
- Manifest tool absent → add item: `type: delete-workflow`, `modified: false`,
  `path: .forge/workflows/{old-name}.md`,
  `label: ".forge/workflows/{old-name}.md — retired workflow, cannot verify modifications"`,
  `action: "Delete old workflow"`

**5b-portability — Legacy model fields in workflows.** Scan all `.md` files
in `.forge/workflows/` for the pattern `^model:\s+.+$` (multiline).

For each file containing legacy `model:` fields, add item:
`type: legacy-model-field`, `required: false`,
`path: .forge/workflows/{filename}`,
`label: ".forge/workflows/{filename} — legacy model: field detected"`,
`action: "Will be auto-migrated by /forge:rebuild workflows"`

**5b-rename — Retired command names in pipeline config.** Scan every configured
pipeline for phases that use retired built-in command names.

Retired command name map:

| Retired command | Role | Replacement |
|----------------|------|-------------|
| `engineer` | `plan` | `plan` |
| `supervisor` | `review-plan` | `review-plan` |
| `supervisor` | `review-code` | `review-code` |

Read all pipelines:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" list-pipelines 2>/dev/null
```

For each pipeline, read its full phase list:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" pipeline get {NAME}
```

For each phase with a retired command, add item:
`type: update-pipeline-cmd`, `required: true`,
`path: pipeline "{name}" phase {N}`,
`label: "pipeline \"{name}\" phase {N} — command \"{cmd}\" is retired"`,
`action: "Rename command to \"{replacement}\""`

Do not match custom commands like `supervisor-code` or `engineer-security`.

**5c — Missing `paths.customCommands` key.**

```sh
node "$FORGE_ROOT/tools/manage-config.cjs" get paths.customCommands 2>/dev/null
```

If the key is missing from config, add item:
`type: add-paths-key`, `required: true`,
`path: .forge/config.json`,
`label: "New config field available: paths.customCommands"`,
`action: "Add paths.customCommands with default \"engineering/commands\""`

**5d/5e — Custom phase workflow field audit.** For each pipeline, identify
phases whose `command` is not in the built-in list
(`plan`, `review-plan`, `implement`, `review-code`, `validate`, `approve`, `commit`).

For each custom phase where `workflow` field is NOT set, check both locations:
```sh
ls engineering/commands/{cmd}.md 2>/dev/null && echo "found:engineering"
ls .claude/commands/{cmd}.md 2>/dev/null && echo "found:claude"
```

- Found in exactly one location → add item:
  `type: add-workflow-field`, `required: true`,
  `path: pipeline "{name}" phase {N}`,
  `label: "pipeline \"{name}\" phase {N} (role: {role}) — no workflow field, file found at {path}"`,
  `action: "Add workflow field: \"{found_path}\""`

- Found in both locations → add item:
  `type: add-workflow-field`, `required: true`,
  `path: pipeline "{name}" phase {N}`,
  `label: "pipeline \"{name}\" phase {N} (role: {role}) — no workflow field, file found in two locations"`,
  `action: "Add workflow field (user chooses which path)"`

- Not found → add item:
  `type: missing-command-file`, `required: false`,
  `path: command "{cmd}"`,
  `label: "pipeline \"{name}\" phase {N} — command file missing for \"{cmd}\""`,
  `action: "Run /forge:add-pipeline to create it, or create manually"`

**5g — Transient store gitignore audit.** `.forge/store/events/` accumulates
one JSON per agent phase per task or bug — these are transient logs that
should not be committed. Detect projects where the path is not yet ignored.

1. Check whether the project root has a `.gitignore`:
   ```sh
   ls .gitignore 2>/dev/null
   ```
   If absent, skip this sub-check entirely (no items added — the project is
   not under git or has no gitignore convention; do not auto-create).

2. Read `.gitignore` and search for any of these match strings on a
   non-comment, non-blank line (literal substring match):
   - `.forge/store/events/`
   - `.forge/store/events`
   - `.forge/store/`
   - `.forge/`

   If any match → already ignored, no item added.

3. If no match → add item:
   `type: add-gitignore-entry`, `required: true`,
   `path: .gitignore`,
   `label: ".gitignore — .forge/store/events/ is not ignored (transient agent event logs)"`,
   `action: "Append .forge/store/events/ to .gitignore"`

This sub-check runs whether or not pipelines are configured — it depends
only on `.gitignore` and the project's `.forge/` layout (always present
post-init).

**5f — Persona decoration.** For each command file found during 5d/5e that
has a `## Persona` section but no symbol line (first non-blank line after the
heading does not start with one of `🌱 🌿 ⛰️ 🌊 🍂 🍃`), add item:

Check manifest status:
```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" check {filepath}
```

- Unmodified → `modified: false`
- Modified (exit 1) → `modified: true`, flag with `△` in label

`type: add-persona-symbol`, `required: false`,
`path: {filepath}`,
`label: "{filepath} — Persona section, no symbol line (optional)"`,
`action: "Add: {symbol} **{Project} {name}** — {announcement}"`

Symbol is derived from the phase role:
- `plan` / `implement` / `commit` → 🌱
- `review-plan` / `review-code` → 🌿
- `approve` → ⛰️
- any other → 🌿

**Pipeline gate behavior:** All sub-checks always run. Pipeline-dependent
sub-checks (5b-rename, 5c, 5d, 5e, 5f) naturally produce zero items when
no pipelines are configured. Project-wide sub-checks (5b-pre, 5b-portability,
5g) run regardless of pipeline state. If `.forge/config.json` does not exist,
Step 5 is skipped entirely (handled by 5a above).

### 5b-present — Present consolidated prompt

If `AUDIT_ITEMS` is empty:
> 〇 Pipeline audit complete — nothing to update.

Skip to **Step 6**.

If `AUDIT_ITEMS` has entries, separate them into required and optional groups
and emit:

```
## Step 5 — Audit (N items)

  [1] △ .claude/commands/supervisor.md — retired name, user edits detected.
         Merge into review-plan.md before deleting. Delete old file?
  [2] 〇 .forge/workflows/architect_sprint_plan.md — legacy model: field detected.
         Will be auto-migrated by /forge:rebuild workflows. No action needed.
  [3] 〇 pipeline "main" phase 3 — no workflow field.
         Add: "workflow": "engineering/commands/qa.md"
  [4] 〇 pipeline "main" phase 4 — command file missing.
         No file found for command "security-check". Create via /forge:add-pipeline.
  [5] 〇 engineering/commands/qa.md — Persona section, no symbol line. (optional)
         Add: 🌿 **QA Engineer** — I validate implementations...

  Apply required? [Y]  Apply all (including optional)? [a]  Review individually [r]  Skip [n]
```

**Ordering within the list:**

1. Deletion items (`delete-command`, `delete-workflow`) first — highest urgency
2. Pipeline config updates (`update-pipeline-cmd`, `add-paths-key`) second
3. Workflow field additions (`add-workflow-field`) third
4. Gitignore entries (`add-gitignore-entry`) fourth — repo hygiene
5. Missing file warnings (`missing-command-file`) fifth — always advisory
6. Legacy model field items (`legacy-model-field`) sixth — auto-applied
7. Optional decoration items (`add-persona-symbol`) last — marked `(optional)`

**Behavior for each choice:**

---

**[Y] — Apply required items:**

For each item where `required: true`:
- If `modified: true` (△): prompt individually for that specific item before
  acting. Use the original prompt text from the corresponding sub-check
  (5b-pre for deletions, 5b-rename for command renames, 5c for paths key,
  5e for workflow fields).
- If `modified: false`: apply automatically, emit `  〇 applied: <label>`

For `legacy-model-field` items (`required: false`): acknowledge automatically,
emit `  〇 acknowledged: <label>`

For `add-persona-symbol` items: skip, emit
`  ── skipped: <label> (optional)`

For `missing-command-file` items: emit advisory reminder at the end.

**`add-gitignore-entry` apply rule:** append the following two lines to
`.gitignore` (preserve existing trailing newline; do not introduce a
duplicate if either line is already present after a re-read):

```
# Forge — transient agent event logs (one file per phase, do not commit)
.forge/store/events/
```

Touch nothing else in `.gitignore`. Emit `〇 applied: appended .forge/store/events/ to .gitignore`.

After processing, if any optional items were skipped:
```
  ── N optional decoration item(s) skipped (re-run with [a] to include, or [r] for individual review)
```

---

**[a] — Apply all including optional:**

Same as [Y] for all required items, plus apply `add-persona-symbol` items.
Each decoration is applied by prepending the symbol line immediately after the
`## Persona` heading in the target file. Touch **nothing else** in the file
— not punctuation, not spacing, not other lines.

For `modified: true` files targeted for decoration, show the diff before applying:
```
  △ {filepath} has been manually modified — your edits will be preserved.
    The decoration only adds one line after ## Persona.
```

---

**[r] — Review individually:**

Fall back to the original per-item behavior. Walk through each item in order
using the existing prompts from the corresponding sub-checks:

- `delete-command` / `delete-workflow` items → follow 5b-pre prompts
- `legacy-model-field` items → follow 5b-portability acknowledgment prompt
- `update-pipeline-cmd` items → follow 5b-rename diff preview and confirmation
- `add-paths-key` items → follow 5c prompt
- `add-workflow-field` items → follow 5e audit (Case A, B, or C)
- `missing-command-file` items → follow 5e Case C prompt
- `add-gitignore-entry` items → confirm append, then write the entry per 5g
- `add-persona-symbol` items → follow 5f persona decoration prompt

This preserves backward compatibility exactly — each item type uses the same
prompt text and confirmation flow as the original sub-checks.

---

**[n] — Skip all:**

Emit summary of skipped items:
```
  ── N item(s) skipped:
  ── [1] <label>
  ── [2] <label>
  ...
```

For any `legacy-model-field` items in the skipped list, add:
```
  △ Some workflows may not resolve models correctly until regenerated.
```

Proceed to **Step 6**.

---

### Key behavioral invariants

- `△` items (user-modified files) must never be deleted without explicit confirmation
  even in bulk-apply mode — if `[Y]` or `[a]` is chosen and an item is `modified: true`,
  prompt for that specific item before acting
- `missing-command-file` items are always advisory — never blocked, always result
  in a reminder at the end
- `legacy-model-field` items are auto-acknowledged in `[Y]` and `[a]` modes (no
  individual prompt), but shown in the list for transparency
- `add-persona-symbol` items are excluded from `[Y]` bulk-apply but included in `[a]`
- The individual review mode `[r]` must behave identically to the original
  per-item prompts from 5b-pre, 5b-portability, 5b-rename, 5c, 5e, and 5f
- If `.forge/config.json` does not exist, Step 5 is skipped entirely
- If `AUDIT_ITEMS` is empty (no findings at all), print the "nothing to update"
  message and proceed to Step 6

---

## Step 6 — Record state and summarise

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 6 7 "Record state" drift
```

> **Note:** `paths.forgeRef` was already written at the start
> of Step 4. Step 6 does not repeat that write — it records migration state only.

**Write `.forge/update-check-cache.json`** to record the completed migration.
Read the existing file if present, update `migratedFrom`, `localVersion`,
`distribution`, `forgeRef`, `updateStatus`, `pendingReason`, and
`pendingMigrations`, then write it back. Use the Write or Edit tool — do not run
a shell command for this step. The `.forge/` directory always exists at this
point (it was checked earlier), so no `mkdir -p` is needed.

If the file does not exist, create it with:
```json
{
  "migratedFrom": "<LOCAL_VERSION>",
  "localVersion": "<LOCAL_VERSION>",
  "distribution": "<DISTRIBUTION>",
  "forgeRef": "<LOCAL_VERSION>",
  "updateStatus": "complete",
  "pendingReason": null,
  "pendingMigrations": []
}
```

On the Update Complete path, `updateStatus` is `"complete"`, `pendingReason` is
`null`, and `pendingMigrations` is `[]`. On the Pending path (Step 4), the
fields are set differently — see the migration completion gate section.

Print the final summary:

```
## 〇 Forge {LOCAL_VERSION} — Update Complete

{if install happened:}
  Plugin updated: {old version} → {LOCAL_VERSION}
{end if}

{if migrations applied:}
  Migrations applied: {baseline} → {LOCAL_VERSION}
  Regenerated: {list of targets}
{end if}

{if custom command audit ran:}
  Pipeline audit: {N} phase(s) reviewed
  {if workflow fields added:}  〇 workflow fields added: {list of pipeline/phase}  {end if}
  {if files missing:}  △ command files still needed: {list}  {end if}
{end if}

── Next steps:
   • Run /forge:health to verify knowledge base currency
   • Generated workflows and tools are ready to use
   {if files missing:}• Run /forge:add-pipeline to create missing command file(s){end if}

  Subagent isolation: {used | bypassed (inline)}
```

---

## Step 7 — Link KB to Agent Instruction Files

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 7 7 "Tomoshibi" lumen
```

Invoke Tomoshibi to ensure every coding-agent instruction file in the project
has up-to-date links to the Forge knowledge base and generated workflow entry points.

Use the Skill tool:
  skill: "forge:refresh-kb-links"

---

## Arguments

$ARGUMENTS

| Argument | Purpose |
|----------|---------|
| `--from <version>` | Override the migration baseline (useful when cache is missing or user jumped versions) |
| `--skip-check` | Skip the remote version check — only apply pending migrations from cache |

---

## On error

If any step above fails unexpectedly, describe what went wrong and ask:

> "This looks like a Forge bug. Would you like to file a report to help improve it? Run `/forge:report-bug` — I'll pre-fill the report from this conversation."
