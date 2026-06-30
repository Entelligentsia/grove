# Generation: Tools

## Purpose

Vendor the plugin tools closure and validation schemas into the project so
generated workflows can invoke tools via `node .forge/tools/<tool>.cjs` from
the project root without resolving `$FORGE_ROOT` at runtime.

Store validation schemas are loaded at runtime from `.forge/schemas/`
(project-installed), `forge/schemas/` (in-tree fallback), or
`$FORGE_ROOT/schemas/` (plugin-installed fallback). During init, schemas
are copied to `.forge/schemas/` so that validation works without relying
on fallback paths.

## Inputs

- `.forge/config.json` — target paths

## Outputs

- `.forge/tools/` — vendored plugin tools closure
- `.forge/tools/.forge-tools-version` — version marker for `/forge:health` staleness check
- `.forge/schemas/` — JSON Schema copies from the installed plugin

## Instructions

Read `.forge/config.json` for:
- `paths.store` (default: `.forge/store`)

### Step 1 — Copy validation schemas

Copy all JSON Schema files from the installed plugin to the project:

```sh
mkdir -p .forge/schemas
cp "$FORGE_ROOT/schemas/"*.schema.json .forge/schemas/
```

This ensures `store-cli.cjs` and `validate-store.cjs` can validate records
using the full schema (not the minimal fallback) even when the project is
not inside the Forge source tree.

### Step 2 — Vendor plugin tools

Copy the plugin tools closure into the project's `.forge/tools/` so that
generated artifacts can invoke `node .forge/tools/<tool>.cjs` from the
project root without resolving `$FORGE_ROOT`:

Copy BOTH `.cjs` and `.js` files. Some tools require `.js` helpers at load
time — e.g. `store-cli.cjs` does a top-level `require('./lib/validate.js')`
and `collate.cjs` requires `./lib/result.js` — so a `.cjs`-only copy leaves
`store-cli.cjs` dead-on-arrival and breaks KB collation. `-maxdepth 1`
excludes the `__tests__/` subtree without copying any `*.test.*` files.

```sh
mkdir -p .forge/tools/lib

# Copy top-level tool files (.cjs and .js — e.g. list-skills.js)
find "$FORGE_ROOT/tools" -maxdepth 1 -type f \( -name '*.cjs' -o -name '*.js' \) \
  -exec cp {} .forge/tools/ \;

# Copy lib/ helper files (.cjs and .js — e.g. result.js, validate.js)
find "$FORGE_ROOT/tools/lib" -maxdepth 1 -type f \( -name '*.cjs' -o -name '*.js' \) \
  -exec cp {} .forge/tools/lib/ \;
```

After copying, record each vendored file in the generation manifest so that
`/forge:health` can detect modifications or stale copies:

```sh
for f in $(find .forge/tools .forge/tools/lib -maxdepth 1 -type f \( -name '*.cjs' -o -name '*.js' \)); do
  node "$FORGE_ROOT/tools/generation-manifest.cjs" record "$f"
done
```

### Step 2b — Write version marker

After the tool copy loop, write the version marker so `/forge:health` can
detect whether the vendored tools are stale relative to the active plugin:

```sh
ACTIVE_VERSION=$(node -e "console.log(require('$FORGE_ROOT/.claude-plugin/plugin.json').version)")
node -e "
const fs = require('fs');
fs.writeFileSync('.forge/tools/.forge-tools-version', JSON.stringify({ version: '${ACTIVE_VERSION}' }) + '\n');
"
```

### Step 3 — Verify

```sh
node "$FORGE_ROOT/tools/validate-store.cjs" --dry-run
```

If it exits non-zero, report the error. Do not proceed to Phase 9 until this passes.

### Step 4 — Register the Forge root

Write the project-relative Forge root into config. In the CLI-first vendored
world the `.forge/` directory IS the Forge root (tools, schemas, hooks, init,
meta are all vendored there) — NEVER write an absolute path (plugin cache,
npm global payload, …): absolute paths break on version upgrades, nvm/node
switches, and machine moves.

```sh
node .forge/tools/manage-config.cjs set paths.forgeRoot '".forge"'
```

### Step 5 — Record hashes

Record all generated artifacts in the generation manifest so health checks
can detect later modifications:

```sh
for f in .forge/schemas/*.schema.json; do
  node "$FORGE_ROOT/tools/generation-manifest.cjs" record "$f"
done
node "$FORGE_ROOT/tools/generation-manifest.cjs" record .forge/config.json
```

## Notes

- `/forge:update` automatically refreshes schemas and re-vendors tools as part
  of its normal flow — run it after upgrades to pick up any changed tools or
  schema updates from the new version.
- Generated workflow files invoke tools using the vendored project-relative path:
  ```
  node .forge/tools/<tool>.cjs
  ```
  This works from the project root without resolving `$FORGE_ROOT` at runtime.
- `paths.forgeRef` in config records the plugin version the project was generated
  against. `forge-preflight.cjs` uses it to resolve the plugin root via cache
  lookup when runtime telemetry requires the original plugin path.