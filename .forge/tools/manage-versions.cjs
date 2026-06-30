#!/usr/bin/env node
'use strict';

// Forge tool: manage-versions
// Manage .forge/structure-versions.json — snapshot lifecycle for structural elements.
//
// Subcommands:
//   init         Write snapshot 0 at first init (idempotent: no-op if file exists)
//   current      Print current snapshot index and metadata
//   list         Print tabular summary of all snapshots
//   add-snapshot Archive current structural elements and record a new snapshot entry
//
// Composition model: Working Artifact = base@pluginVersion + snapshot@currentSnapshot + user_enhancements
// Snapshot-array invariant: snapshots is ordered ascending by index; currentSnapshot always equals
//   snapshots[snapshots.length - 1].index.
//
// Usage:
//   node manage-versions.cjs init [--dry-run]
//   node manage-versions.cjs current
//   node manage-versions.cjs list
//   node manage-versions.cjs add-snapshot --source <source> [--enhanced-elements <csv>] [--dry-run]
//     --source <string>              Required. One of: post-init | post-sprint:<ID> | on-demand
//     --enhanced-elements <csv>      Optional. Comma-separated list of .forge/-relative paths that were enhanced.
//     --dry-run                      Log intent without performing I/O.
//
// Environment:
//   FORGE_ROOT — path to forge plugin root (used by init to locate plugin.json and schemas)
//   Falls back to auto-detection via ../../ from __dirname when FORGE_ROOT is unset.

const fs = require('fs');
const path = require('path');
const { resolveForgeRoot } = require('./lib/forge-root.cjs');

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Relative path suffix for structure-versions.json (from project root)
const VERSIONS_SUFFIX = path.join('.forge', 'structure-versions.json');

// Default project root is cwd when used as CLI; exports allow test injection.
const VERSIONS_PATH = path.join(process.cwd(), VERSIONS_SUFFIX);

// ---------------------------------------------------------------------------
// Exported helpers (used by unit tests and callers)
// ---------------------------------------------------------------------------

/**
 * Resolve the path to structure-versions.json for the given project root.
 * @param {string} projectRoot
 * @returns {string}
 */
function versionsPath(projectRoot) {
  return path.join(projectRoot, VERSIONS_SUFFIX);
}

/**
 * Read and parse .forge/structure-versions.json.
 * @param {string} projectRoot
 * @returns {object} parsed document
 * @throws {Error} when file does not exist or cannot be parsed
 */
function readStructureVersions(projectRoot) {
  const filePath = versionsPath(projectRoot);
  if (!fs.existsSync(filePath)) {
    throw new Error(`structure-versions.json not found at ${filePath}. Run \`manage-versions init\` first.`);
  }
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch (e) {
    throw new Error(`Failed to parse structure-versions.json at ${filePath}: ${e.message}`);
  }
}

/**
 * Write data to .forge/structure-versions.json atomically via .tmp.PID rename.
 * @param {string} projectRoot
 * @param {object} data
 */
function writeStructureVersions(projectRoot, data) {
  const filePath = versionsPath(projectRoot);
  const json = JSON.stringify(data, null, 2) + '\n';
  const tmp = filePath + '.tmp.' + process.pid;
  try {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(tmp, json, 'utf8');
    fs.renameSync(tmp, filePath);
  } catch (e) {
    try { fs.unlinkSync(tmp); } catch {}
    throw new Error(`Failed to write structure-versions.json: ${e.message}`);
  }
}

/**
 * Resolve forge root from env or __dirname fallback.
 * Delegates to the shared forge-root.cjs helper (FR-001).
 * @param {string} [envForgeRoot] - value of FORGE_ROOT env var
 * @returns {string}
 */

/**
 * Read the plugin version from FORGE_ROOT/.claude-plugin/plugin.json.
 * @param {string} forgeRoot
 * @returns {string}
 */
function readPluginVersion(forgeRoot) {
  const pluginPath = path.join(forgeRoot, '.claude-plugin', 'plugin.json');
  try {
    const pkg = JSON.parse(fs.readFileSync(pluginPath, 'utf8'));
    if (!pkg.version) throw new Error('version field missing');
    return pkg.version;
  } catch (e) {
    throw new Error(`Failed to read plugin version from ${pluginPath}: ${e.message}`);
  }
}

/**
 * Read the overlay tool version from FORGE_ROOT/schemas/project-overlay.schema.json.
 * Falls back to "1.0.0" if the schema does not contain a version field.
 * @param {string} forgeRoot
 * @returns {string}
 */
function readOverlayToolVersion(forgeRoot) {
  const schemaPath = path.join(forgeRoot, 'schemas', 'project-overlay.schema.json');
  try {
    const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
    if (typeof schema.version === 'string' && schema.version) {
      return schema.version;
    }
  } catch {
    // Schema unreadable — fall back
  }
  return '1.0.0';
}

// Structural element directories that are eligible to be archived.
const STRUCTURAL_ELEMENT_DIRS = ['personas', 'skills', 'workflows', 'templates'];

/**
 * Copy a file, creating intermediate directories as needed.
 * @param {string} src
 * @param {string} dest
 */
function copyFileWithDirs(src, dest) {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
}

/**
 * Add a new snapshot entry to structure-versions.json and archive the
 * current structural elements listed in enhancedElements.
 *
 * @param {string} projectRoot        - path to the project root (where .forge/ lives)
 * @param {string} source             - snapshot source label (post-init | post-sprint:<ID> | on-demand)
 * @param {string[]} enhancedElements - list of paths that were enhanced. Both
 *                                       ".forge/-relative" (e.g. "personas/X.md") and
 *                                       project-root-relative (".forge/personas/X.md")
 *                                       forms accepted; the tool strips a leading
 *                                       ".forge/" if present. See forge#108.
 * @param {boolean} [dryRun]          - when true, log intent but perform no I/O
 */
function addSnapshot(projectRoot, source, enhancedElements, dryRun) {
  if (!source) {
    throw new Error('--source is required for add-snapshot. Provide one of: post-init | post-sprint:<ID> | on-demand');
  }

  const doc = readStructureVersions(projectRoot);
  const nextIndex = doc.currentSnapshot + 1;
  const archivePath = path.join('.forge', 'archive', `snap-${nextIndex}`);
  const archiveAbsPath = path.join(projectRoot, archivePath);

  // Guard: fail if archive directory already exists to prevent corruption.
  if (fs.existsSync(archiveAbsPath)) {
    throw new Error(
      `Archive directory already exists: ${archiveAbsPath} (snap-${nextIndex}). ` +
      'Cannot create snapshot — remove or rename the existing archive directory first.'
    );
  }

  const createdAt = new Date().toISOString();
  const newSnapshot = {
    index: nextIndex,
    createdAt,
    source,
    enhancedElements: enhancedElements || [],
    archivePath
  };

  if (dryRun) {
    console.log(`[dry-run] Would create archive at: ${archiveAbsPath}`);
    console.log(`[dry-run] Would archive ${(enhancedElements || []).length} element(s).`);
    console.log(`[dry-run] Would write snapshot entry:`);
    console.log(JSON.stringify(newSnapshot, null, 2));
    return;
  }

  // Archive each enhanced element by copying from .forge/ into the archive dir.
  //
  // Workflow callers pass paths in two forms historically:
  //   (a) ".forge/-relative", e.g. "personas/engineer.md"
  //   (b) project-root-relative, e.g. ".forge/personas/engineer.md"
  // The docstring at the top of this function declared (a), but every meta-enhance
  // and base-pack/enhance.md invocation passes (b). The previous code did
  // path.join(projectRoot, ".forge", relPath) which double-prefixed (b) paths
  // and silently skipped them via fs.existsSync. Result: every archive directory
  // since basePackVersion 0.43.3 was created empty — layer 2 of the composition
  // contract (manage-versions.cjs:13) became a no-op. See forge#108 / FORGE-BUG-038.
  //
  // Fix: normalize each relPath by stripping a leading ".forge/" if present.
  // Archive destination uses the normalized form for consistency.
  for (const relPath of (enhancedElements || [])) {
    const normalizedRel = relPath.replace(/^\.\/?(?=\.forge\/)/, '').replace(/^\.forge\//, '');
    const srcPath = path.join(projectRoot, '.forge', normalizedRel);
    const destPath = path.join(archiveAbsPath, normalizedRel);
    if (fs.existsSync(srcPath)) {
      copyFileWithDirs(srcPath, destPath);
    }
    // If the source file does not exist, skip silently — the element list
    // may reference files that were removed or renamed; archiving what's there
    // is better than failing the whole snapshot.
  }

  // Also create the archive directory even if no elements were listed,
  // so archivePath references a real directory.
  fs.mkdirSync(archiveAbsPath, { recursive: true });

  // Append snapshot entry and advance currentSnapshot.
  doc.snapshots.push(newSnapshot);
  doc.currentSnapshot = nextIndex;
  writeStructureVersions(projectRoot, doc);

  console.log(`ノ add-snapshot complete — snapshot ${nextIndex} written (source: ${source}, elements: ${(enhancedElements || []).length})`);
}

/**
 * Initialise structure-versions.json with snapshot 0.
 * Idempotent: if the file already exists, exits cleanly without overwriting.
 *
 * @param {string} projectRoot - path to the project root (where .forge/ lives)
 * @param {string} forgeRoot   - path to the forge plugin root
 * @param {boolean} [dryRun]   - when true, log intent but perform no I/O
 * @param {string} [source]    - source label for snapshot 0 (default: 'base-pack')
 */
function initStructureVersions(projectRoot, forgeRoot, dryRun, source) {
  const filePath = versionsPath(projectRoot);

  // Idempotency: if the file already exists, do nothing.
  if (fs.existsSync(filePath)) {
    console.log(`〇 structure-versions.json already exists — skipping (idempotent).`);
    return;
  }

  const effectiveSource = source || 'base-pack';
  const basePackVersion = readPluginVersion(forgeRoot);
  const overlayToolVersion = readOverlayToolVersion(forgeRoot);

  const doc = {
    basePackVersion,
    overlayToolVersion,
    currentSnapshot: 0,
    snapshots: [
      {
        index: 0,
        createdAt: new Date().toISOString(),
        source: effectiveSource,
        enhancedElements: [],
        archivePath: null
      }
    ]
  };

  if (dryRun) {
    console.log('[dry-run] Would write structure-versions.json:');
    console.log(JSON.stringify(doc, null, 2));
    return;
  }

  writeStructureVersions(projectRoot, doc);
  console.log(`ノ structure-versions.json written (snapshot 0, source: ${effectiveSource}, plugin: v${basePackVersion})`);
}

// ---------------------------------------------------------------------------
// forge#107 — Approach A layer 3: snapshot replay
//
// After /forge:regenerate writes fresh base-pack content over .forge/{personas,
// skills,workflows,templates}/, walk the snapshots in order and restore each
// enhancedElement that matches the target prefix. Later snapshots win on file
// collision (last write).
//
// Semantics: "overlay" — user-enhanced files retain their captured content
// even when the base-pack version of that file has changed in a plugin
// update. Trade-off accepted for v1; future v2 may layer 3-way merge.
//
// Target prefix is normalized (leading ".forge/" stripped) and matches against
// the normalized form of each enhancedElement's path. So:
//   --target personas              matches  personas/X.md, personas/Y.md
//   --target personas/engineer.md  matches only that exact file
//   --target .forge/personas       same as --target personas
// ---------------------------------------------------------------------------

/**
 * Normalize a path by stripping a leading ".forge/" prefix.
 * Symmetric with addSnapshot's archive normalization (forge#108).
 */
function normalizeRel(p) {
  return p.replace(/^\.\/?(?=\.forge\/)/, '').replace(/^\.forge\//, '');
}

/**
 * Replay snapshots — restore enhanced files matching the target prefix.
 *
 * @param {string} projectRoot - cwd; project root with .forge/
 * @param {string} target       - prefix (e.g. "personas") or exact path
 *                                (e.g. "personas/engineer.md"); ".forge/" prefix tolerated
 * @param {boolean} [dryRun]    - log intent without copying
 * @returns {{restored: string[], skipped: string[]}}
 */
function replaySnapshots(projectRoot, target, dryRun) {
  if (!target) {
    throw new Error('replay requires --target <prefix>. Examples: --target personas, --target personas/engineer.md');
  }

  const normalizedTarget = normalizeRel(target);
  const doc = readStructureVersions(projectRoot);

  // Build a per-file map: relPath -> { archivePath, snapshotIndex }.
  // Later snapshots overwrite earlier on collision (Map insertion semantics
  // give us first-set value; we want last-set, so iterate ascending and
  // overwrite explicitly).
  const fileMap = new Map();
  for (const snap of doc.snapshots) {
    if (!snap.enhancedElements || !snap.archivePath) continue;
    for (const elem of snap.enhancedElements) {
      const normalized = normalizeRel(elem);
      // Match: target is a path-prefix of normalized element.
      // Path-boundary: ensure "personas" matches "personas/X.md" but not "personas-foo.md".
      const isMatch = normalized === normalizedTarget ||
                      normalized.startsWith(normalizedTarget + '/');
      if (!isMatch) continue;

      const archiveAbs = path.join(projectRoot, snap.archivePath, normalized);
      fileMap.set(normalized, { archiveAbs, snapshotIndex: snap.index });
    }
  }

  const restored = [];
  const skipped = [];

  for (const [normalized, { archiveAbs, snapshotIndex }] of fileMap) {
    const destAbs = path.join(projectRoot, '.forge', normalized);
    if (!fs.existsSync(archiveAbs)) {
      // Archive missing (pre-forge#108 snapshots created empty archives).
      // Skip — we can't restore what we don't have.
      skipped.push(normalized);
      continue;
    }
    if (dryRun) {
      console.log(`[dry-run] Would restore ${normalized} from snap-${snapshotIndex}`);
      restored.push(normalized);
      continue;
    }
    copyFileWithDirs(archiveAbs, destAbs);
    restored.push(normalized);
  }

  if (restored.length > 0) {
    console.log(`〇 replay complete — ${restored.length} file(s) restored from snapshots matching '${target}'`);
  } else if (skipped.length > 0) {
    console.log(`△ replay — ${skipped.length} matching element(s) skipped (archive missing; created before forge#108 fix)`);
  } else {
    console.log(`〇 replay — no enhanced elements match target '${target}'; nothing to restore`);
  }

  return { restored, skipped };
}

// ---------------------------------------------------------------------------
// Exports (for unit tests)
// ---------------------------------------------------------------------------

module.exports = {
  initStructureVersions,
  addSnapshot,
  replaySnapshots,
  readStructureVersions,
  writeStructureVersions,
  VERSIONS_PATH,
  versionsPath,
  readPluginVersion,
  readOverlayToolVersion,
  normalizeRel,
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

if (require.main === module) {

  process.on('uncaughtException', (error) => {
    console.error('Fatal manage-versions error:', error.message);
    process.exit(1);
  });

  const args = process.argv.slice(2);
  const DRY_RUN = args.includes('--dry-run');
  const subcommand = args.find(a => !a.startsWith('--'));

  const forgeRoot = resolveForgeRoot(process.env.FORGE_ROOT);
  const projectRoot = process.cwd();

  try {
    switch (subcommand) {
      case 'init': {
        const sourceIdx = args.indexOf('--source');
        const initSource = sourceIdx !== -1 ? args[sourceIdx + 1] : undefined;
        initStructureVersions(projectRoot, forgeRoot, DRY_RUN, initSource);
        break;
      }

      case 'current': {
        const doc = readStructureVersions(projectRoot);
        const snap = doc.snapshots.find(s => s.index === doc.currentSnapshot);
        console.log(`Current snapshot: ${doc.currentSnapshot}`);
        if (snap) {
          console.log(`  source:    ${snap.source}`);
          console.log(`  createdAt: ${snap.createdAt}`);
          console.log(`  plugin:    v${doc.basePackVersion}`);
        }
        break;
      }

      case 'list': {
        const doc = readStructureVersions(projectRoot);
        console.log(`Snapshots (current: ${doc.currentSnapshot}):`);
        console.log(`${'#'.padEnd(4)} ${'source'.padEnd(20)} ${'createdAt'.padEnd(28)} elements`);
        console.log('-'.repeat(70));
        for (const snap of doc.snapshots) {
          const current = snap.index === doc.currentSnapshot ? '*' : ' ';
          const elems = snap.enhancedElements ? snap.enhancedElements.length : 0;
          console.log(`${current}${String(snap.index).padEnd(3)} ${snap.source.padEnd(20)} ${snap.createdAt.padEnd(28)} ${elems}`);
        }
        break;
      }

      case 'add-snapshot': {
        // Parse --source flag
        const sourceIdx = args.indexOf('--source');
        const source = sourceIdx !== -1 ? args[sourceIdx + 1] : null;
        if (!source || source.startsWith('--')) {
          console.error('× add-snapshot requires --source <value>.');
          console.error('  Accepted values: post-init | post-sprint:<SPRINT_ID> | on-demand');
          process.exit(1);
        }

        // Parse optional --enhanced-elements flag (comma-separated list)
        const elementsIdx = args.indexOf('--enhanced-elements');
        let enhancedElements = [];
        if (elementsIdx !== -1) {
          const raw = args[elementsIdx + 1];
          if (raw && !raw.startsWith('--')) {
            enhancedElements = raw.split(',').map(s => s.trim()).filter(Boolean);
          }
        }

        addSnapshot(projectRoot, source, enhancedElements, DRY_RUN);
        break;
      }

      case 'replay': {
        // forge#107 — Approach A layer 3
        const targetIdx = args.indexOf('--target');
        const target = targetIdx !== -1 ? args[targetIdx + 1] : null;
        if (!target || target.startsWith('--')) {
          console.error('× replay requires --target <prefix>.');
          console.error('  Examples: --target personas, --target personas/engineer.md');
          process.exit(1);
        }
        replaySnapshots(projectRoot, target, DRY_RUN);
        break;
      }

      default: {
        console.error(`× Unknown subcommand: ${subcommand || '(none)'}`);
        console.error('  Usage: manage-versions.cjs <init|current|list|add-snapshot|replay> [--dry-run]');
        process.exit(1);
      }
    }
  } catch (err) {
    console.error(`× manage-versions error: ${err.message}`);
    process.exit(1);
  }
}
