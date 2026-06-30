# Phase 3 — Materialize

**Deliverable:** `.forge/{personas,skills,workflows,templates}` fully populated.

This phase is deterministic and requires no LLM calls.

Set `$FORGE_ROOT` and resolve `$KB_PATH`:

```sh
KB_PATH=$(node -e "try{console.log(require('./.forge/config.json').paths.engineering)}catch{console.log('engineering')}")
```

---

## Phase gate — verify Phase 1 foundation

Before starting any work, verify the Phase 1 foundation:

```sh
node "$FORGE_ROOT/tools/verify-phase.cjs" --phase 1 --foundation-only
```

If this exits 1, halt. `project.name` and `project.prefix` are required by
`substitute-placeholders.cjs`. Missing them causes a hard failure.

---

## Steps (follow in order)

### Step 1 — Render phase header

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 3 4 "Materialize" ember
```

### Step 2 — Build project brief (first pass)

```sh
node "$FORGE_ROOT/tools/build-init-context.cjs" \
  --config    .forge/config.json \
  --personas  .forge/personas \
  --templates .forge/templates \
  --kb        "$KB_PATH" \
  --out       .forge/init-context.md \
  --json-out  .forge/init-context.json
```

The script prints `〇 Brief written — N personas, M templates, K architecture docs` on success.
If it exits non-zero, halt and surface the error.

Note: `.forge/personas/` and `.forge/templates/` are empty at this point —
the brief will have stub entries. The full content is produced in Step 3 below.

### Step 3 — Substitute placeholders (base-pack materialisation)

```sh
node "$FORGE_ROOT/tools/substitute-placeholders.cjs" \
  --forge-root "$FORGE_ROOT" \
  --base-pack  "$FORGE_ROOT/init/base-pack" \
  --config     .forge/config.json \
  --context    .forge/project-context.json \
  --out        .
```

Output directories (managed by the tool's `SUBDIR_OUTPUT_MAP`):
- `base-pack/personas/`     → `.forge/personas/`
- `base-pack/skills/`       → `.forge/skills/`
- `base-pack/workflows/`    → `.forge/workflows/`
- `base-pack/templates/`    → `.forge/templates/`
- `base-pack/workflows-js/` → `.claude/workflows/` (JS orchestration workflows — FORGE-S28-T01)

> Commands are no longer materialised here. Since FORGE-S32-T06 the unified
> slash-command tree (`forge/forge/commands/`) is installed verbatim to
> `.claude/commands/forge/` by the bootstrap manifest's single `commands` entry —
> base-pack no longer carries a `commands/` subdir.

### Step 3a — Record generated JS workflows in the generation manifest

After `substitute-placeholders.cjs` writes the JS workflows, record them in the generation
manifest so `/forge:rebuild` and `/forge:health` can detect stale or missing copies:

```sh
node "$FORGE_ROOT/tools/generation-manifest.cjs" record .claude/workflows/wfl-run-task.js
node "$FORGE_ROOT/tools/generation-manifest.cjs" record .claude/workflows/wfl-run-sprint.js
```

If either `record` call exits non-zero, the file was not written — re-run Step 3 and retry.

If `project-context.json` is absent or missing required keys, halt Phase 3:
```
× Phase 3 aborted: project-context.json is missing or incomplete.
  Ensure Phase 2 (Discover) completed successfully and try again.
```

### Step 4 — build-overlay.cjs smoke test

```sh
node "$FORGE_ROOT/tools/build-overlay.cjs" --task INIT-SMOKE-TEST --format json 2>&1
```

**Expected:** exit 1 with "task not found" error (store not yet seeded). This
is the passing result — it confirms the binary is functional.

Emit a warning when exit code is non-zero (expected):
```
⚠ build-overlay smoke-test: task INIT-SMOKE-TEST not found in store (expected — store not yet seeded). Overlay binary is functional.
```

Phase 3 does NOT halt on this smoke test — result is informational.

Write `.forge/init-progress.json`:
```json
{ "lastPhase": 3, "timestamp": "<current ISO timestamp>" }
```

---

## Verify Phase 3

After writing `init-progress.json`, verify the deliverable:

```sh
node "$FORGE_ROOT/tools/verify-phase.cjs" --phase 3
```

- **Exit 0:** Phase 3 complete. Proceed to Phase 4.
- **Exit 1:** Read the JSON output. This usually means `substitute-placeholders.cjs`
  ran against an incomplete config. Fix `.forge/config.json` and run
  `/forge:rebuild`, or delete `.forge/init-progress.json` and restart.
  **This is a hard failure — do not continue with partial Phase 3.**
