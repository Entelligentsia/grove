#!/usr/bin/env node
'use strict';

// Forge tool: check-structure
// Checks that all files listed in structure-manifest.json are present
// in a project's generated output.
// Usage: node check-structure.cjs [--strict] [--path <project-root>] [--validate-manifest] [--forge-root <path>]
//   --path <project-root>  Directory to check against (default: process.cwd())
//   --strict               Also report files present but NOT in the manifest (extra files)
//   --validate-manifest    Validate manifest against base-pack source files
//   --forge-root <path>    Path to the forge/ plugin directory (required with --validate-manifest)
//
// Reads .forge/config.json paths.* for directory overrides.
// Falls back to manifest dir field if config is absent or unparseable.
//
// Exit 0: all expected files present (or only extras found without --strict)
// Exit 1: any missing files detected; also exit 1 if extras found with --strict

const fs = require('fs');
const path = require('path');
const { getCommandsSubdir } = require('./lib/paths.cjs');

// ── Per-namespace verification logic ──────────────────────────────────────────
//
// Returns { present, missing, extra, total } without calling process.exit.
// - present: number of expected files found
// - missing: array of { nsKey, dir, filename }
// - extra:   array of { nsKey, dir, filename } (populated only when strict=true)
// - total:   total number of expected files across all namespaces

function checkNamespaces(manifest, projectRoot, options = {}) {
  const { strict = false, configPaths = null } = options;

  // Resolve config path overrides
  let resolvedConfigPaths;
  let projectPrefix = '';
  if (configPaths !== null) {
    resolvedConfigPaths = configPaths;
  } else {
    resolvedConfigPaths = {};
    const configFile = path.join(projectRoot, '.forge', 'config.json');
    if (fs.existsSync(configFile)) {
      try {
        const cfg = JSON.parse(fs.readFileSync(configFile, 'utf8'));
        resolvedConfigPaths = (cfg && cfg.paths) ? cfg.paths : {};
        projectPrefix = (cfg && cfg.project && cfg.project.prefix)
          ? getCommandsSubdir(cfg.project.prefix)
          : '';
      } catch {
        // unparseable — use defaults
      }
    }
  }

  let totalPresent = 0;
  let totalExpected = 0;
  const allMissing = [];
  const allExtra = [];

  for (const [nsKey, ns] of Object.entries(manifest.namespaces)) {
    const logicalKey = ns.logicalKey || nsKey;
    const configHasPath = resolvedConfigPaths[logicalKey] && typeof resolvedConfigPaths[logicalKey] === 'string';
    let resolvedDir = configHasPath
      ? resolvedConfigPaths[logicalKey]
      : ns.dir;
    // When config provides the path, use it verbatim (it already includes any
    // prefix subdirectory).  Only apply the prefixed append when falling back
    // to the manifest's base dir.
    if (ns.prefixed && !configHasPath) {
      const suffix = projectPrefix || getCommandsSubdir('forge');
      resolvedDir = resolvedDir + '/' + suffix;
    }
    const absDir = path.join(projectRoot, resolvedDir);

    const expected = ns.files || [];
    totalExpected += expected.length;

    const present = [];
    const missing = [];
    for (const filename of expected) {
      const fullPath = path.join(absDir, filename);
      if (fs.existsSync(fullPath)) {
        present.push(filename);
      } else {
        missing.push({ nsKey, dir: resolvedDir, filename });
      }
    }

    totalPresent += present.length;
    allMissing.push(...missing);

    if (strict) {
      if (fs.existsSync(absDir)) {
        try {
          const expectedSet = new Set(expected);
          const found = fs.readdirSync(absDir);
          for (const f of found) {
            if (!expectedSet.has(f)) {
              allExtra.push({ nsKey, dir: resolvedDir, filename: f });
            }
          }
        } catch {}
      }
    }
  }

  return {
    present: totalPresent,
    missing: allMissing,
    extra: allExtra,
    total: totalExpected,
  };
}

// ── Manifest validation against base-pack ──────────────────────────────────────
//
// Convention-based reverse mapping: each manifest namespace's dir field maps
// to a base-pack source directory. For schemas, the source is forgeRoot/schemas/.
// This function validates that every file in the manifest has a corresponding
// base-pack source, and vice versa.
//
// Returns { manifestOnly, basePackOnly }

function validateManifest(manifest, forgeRoot) {
  const basePackDir = path.join(forgeRoot, 'init', 'base-pack');

  // Convention-based reverse mapping from manifest dir to base-pack source dir
  const nsToSourceDir = {
    personas:  path.join(basePackDir, 'personas'),
    skills:    path.join(basePackDir, 'skills'),
    workflows: path.join(basePackDir, 'workflows'),
    fragments: path.join(basePackDir, 'workflows', '_fragments'),
    templates: path.join(basePackDir, 'templates'),
    commands:  path.join(basePackDir, 'commands'),
    'workflows-js': path.join(basePackDir, 'workflows-js'),
    // tools: not base-pack-sourced — source is forgeRoot/tools/ (verbatim vendor).
    // Uses recursive enumeration to include lib/*.cjs with the lib/ prefix.
    tools: path.join(forgeRoot, 'tools'),
    // schemas: not base-pack-sourced — source is forgeRoot/schemas/
  };

  // Namespaces that require recursive enumeration (files may include path prefixes
  // like lib/*.cjs). The walker returns relative paths with forward slashes.
  const recursiveNs = new Set(['tools']);

  const manifestOnly = [];  // Files in manifest but not in base-pack
  const basePackOnly = [];   // Files in base-pack but not in manifest

  for (const [nsKey, ns] of Object.entries(manifest.namespaces)) {
    // Skip schemas — they ship from forgeRoot/schemas/, not base-pack
    if (nsKey === 'schemas') continue;

    const sourceDir = nsToSourceDir[nsKey];
    if (!sourceDir) continue;

    const manifestFiles = new Set(ns.files || []);

    // Read base-pack source directory (flat or recursive depending on namespace)
    let basePackFiles = [];
    try {
      if (recursiveNs.has(nsKey)) {
        // Recursive walk: enumerate all files, returning paths relative to sourceDir
        // (with forward-slash separators). Excludes *.test.cjs files.
        const walkDir = (dir, prefix) => {
          const entries = fs.readdirSync(dir, { withFileTypes: true });
          for (const entry of entries) {
            const relPath = prefix ? `${prefix}/${entry.name}` : entry.name;
            if (entry.isDirectory()) {
              walkDir(path.join(dir, entry.name), relPath);
            } else if (entry.isFile() && !entry.name.endsWith('.test.cjs')) {
              basePackFiles.push(relPath);
            }
          }
        };
        walkDir(sourceDir, '');
      } else {
        basePackFiles = fs.readdirSync(sourceDir).filter(f => {
          // Only include regular files, skip subdirectories (like _fragments under workflows)
          const stat = fs.statSync(path.join(sourceDir, f));
          return stat.isFile();
        });
      }
    } catch {
      // Directory doesn't exist — all manifest files are manifestOnly
      for (const f of manifestFiles) {
        manifestOnly.push({ nsKey, filename: f, dir: ns.dir });
      }
      continue;
    }

    const basePackFileSet = new Set(basePackFiles);

    // Files in manifest but not in base-pack
    for (const f of manifestFiles) {
      if (!basePackFileSet.has(f)) {
        manifestOnly.push({ nsKey, filename: f, dir: ns.dir });
      }
    }

    // Files in base-pack but not in manifest
    for (const f of basePackFileSet) {
      if (!manifestFiles.has(f)) {
        basePackOnly.push({ nsKey, filename: f, dir: ns.dir });
      }
    }
  }

  return { manifestOnly, basePackOnly };
}

// Checks whether the vendored .forge/tools/ directory is present, and whether
// the version marker (.forge/tools/.forge-tools-version) matches the active
// plugin version from .forge/config.json (paths.forgeRef).
//
// Returns:
//   { present, vendoredVersion, activeVersion, stale, reason }
//
//   present        — true if .forge/tools/ directory exists
//   vendoredVersion — version string from .forge-tools-version (or null)
//   activeVersion   — version string from paths.forgeRef in config (or null)
//   stale          — true when: dir absent=false; marker absent=true; versions differ=true
//   reason         — 'ok' | 'missing' | 'marker-absent' | 'version-mismatch'
function checkToolsVersion(projectRoot) {
  const toolsDir = path.join(projectRoot, '.forge', 'tools');
  const markerFile = path.join(toolsDir, '.forge-tools-version');
  const configFile = path.join(projectRoot, '.forge', 'config.json');

  // Dir absent → not stale (just missing)
  if (!fs.existsSync(toolsDir)) {
    return { present: false, vendoredVersion: null, activeVersion: null, stale: false, reason: 'missing' };
  }

  // Read active version from config
  let activeVersion = null;
  if (fs.existsSync(configFile)) {
    try {
      const cfg = JSON.parse(fs.readFileSync(configFile, 'utf8'));
      activeVersion = (cfg && cfg.paths && cfg.paths.forgeRef) ? cfg.paths.forgeRef : null;
    } catch {
      // unparseable — leave activeVersion null
    }
  }

  // Marker absent → stale
  if (!fs.existsSync(markerFile)) {
    return { present: true, vendoredVersion: null, activeVersion, stale: true, reason: 'marker-absent' };
  }

  // Read vendored version from marker
  let vendoredVersion = null;
  try {
    const marker = JSON.parse(fs.readFileSync(markerFile, 'utf8'));
    vendoredVersion = (marker && marker.version) ? marker.version : null;
  } catch {
    // unparseable marker → treat as marker-absent
    return { present: true, vendoredVersion: null, activeVersion, stale: true, reason: 'marker-absent' };
  }

  // Version mismatch → stale
  if (activeVersion && vendoredVersion && vendoredVersion !== activeVersion) {
    return { present: true, vendoredVersion, activeVersion, stale: true, reason: 'version-mismatch' };
  }

  return { present: true, vendoredVersion, activeVersion, stale: false, reason: 'ok' };
}

// ── Exports ────────────────────────────────────────────────────────────────────

module.exports = { checkNamespaces, validateManifest, checkToolsVersion };

// ── CLI ────────────────────────────────────────────────────────────────────────

if (require.main === module) {
try {
  // ── Parse arguments ──────────────────────────────────────────────────────────

  const argv = process.argv.slice(2);
  let projectRoot = process.cwd();
  let strict = false;
  let validateManifestFlag = false;
  let forgeRoot = null;

  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--path' && argv[i + 1]) {
      projectRoot = path.resolve(argv[++i]);
    } else if (argv[i] === '--strict') {
      strict = true;
    } else if (argv[i] === '--validate-manifest') {
      validateManifestFlag = true;
    } else if (argv[i] === '--forge-root' && argv[i + 1]) {
      forgeRoot = path.resolve(argv[++i]);
    }
  }

  // ── Load structure-manifest.json ─────────────────────────────────────────────

  const manifestPath = forgeRoot
    ? path.join(forgeRoot, 'schemas', 'structure-manifest.json')
    : path.join(__dirname, '..', 'schemas', 'structure-manifest.json');
  if (!fs.existsSync(manifestPath)) {
    process.stderr.write(`× structure-manifest.json not found at ${manifestPath}\n`);
    process.exit(1);
  }

  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  } catch (e) {
    process.stderr.write(`× Failed to parse structure-manifest.json: ${e.message}\n`);
    process.exit(1);
  }

  // ── Validate manifest against base-pack (if requested) ──────────────────────

  if (validateManifestFlag) {
    if (!forgeRoot) {
      process.stderr.write('× --validate-manifest requires --forge-root <path>\n');
      process.exit(1);
    }

    const { manifestOnly, basePackOnly } = validateManifest(manifest, forgeRoot);

    if (manifestOnly.length > 0) {
      process.stdout.write('× Files in manifest but absent from base-pack:\n');
      for (const { nsKey, filename, dir } of manifestOnly) {
        process.stdout.write(`    × ${dir}/${filename} (namespace: ${nsKey})\n`);
      }
    }

    if (basePackOnly.length > 0) {
      process.stdout.write('△ Files in base-pack but absent from manifest:\n');
      for (const { nsKey, filename, dir } of basePackOnly) {
        process.stdout.write(`    △ ${dir}/${filename} (namespace: ${nsKey})\n`);
      }
    }

    if (manifestOnly.length === 0 && basePackOnly.length === 0) {
      process.stdout.write('〇 Manifest and base-pack are in sync.\n');
    }

    if (manifestOnly.length > 0) {
      process.exit(1);
    }
    process.exit(0);
  }

  // ── Check namespaces ─────────────────────────────────────────────────────────

  const result = checkNamespaces(manifest, projectRoot, { strict });

  // ── Format output ────────────────────────────────────────────────────────────

  // Group results by namespace for display
  const byNs = {};
  for (const m of result.missing) {
    if (!byNs[m.nsKey]) byNs[m.nsKey] = { present: 0, missing: [], extra: [] };
    byNs[m.nsKey].missing.push(m.filename);
  }
  for (const e of result.extra) {
    if (!byNs[e.nsKey]) byNs[e.nsKey] = { present: 0, missing: [], extra: [] };
    byNs[e.nsKey].extra.push(e.filename);
  }

  // Count present per namespace
  for (const [nsKey, ns] of Object.entries(manifest.namespaces)) {
    if (!byNs[nsKey]) byNs[nsKey] = { present: 0, missing: [], extra: [] };
    const total = (ns.files || []).length;
    const missingCount = byNs[nsKey].missing.length;
    byNs[nsKey].present = total - missingCount;
  }

  const lines = [];
  let anyMissing = result.missing.length > 0;
  let anyExtra = result.extra.length > 0;

  for (const [nsKey, ns] of Object.entries(manifest.namespaces)) {
    const logicalKey = ns.logicalKey || nsKey;
    const info = byNs[nsKey] || { present: 0, missing: [], extra: [] };
    const total = (ns.files || []).length;
    const dir = ns.dir;

    if (info.missing.length === 0 && info.extra.length === 0) {
      lines.push(`〇 ${dir}/ — ${info.present}/${total} present`);
    } else {
      if (info.missing.length > 0) {
        lines.push(`× ${dir}/ — ${info.present}/${total} present, ${info.missing.length} missing:`);
        for (const f of info.missing) {
          lines.push(`    × ${f}`);
        }
      }
      if (info.extra.length > 0) {
        if (info.missing.length === 0) {
          lines.push(`△ ${dir}/ — ${info.present}/${total} present, ${info.extra.length} extra:`);
        }
        for (const f of info.extra) {
          lines.push(`    △ ${f} (not in manifest)`);
        }
      }
    }
  }

  for (const line of lines) {
    process.stdout.write(line + '\n');
  }

  if (!anyMissing && !anyExtra) {
    process.stdout.write(`〇 Structure check: all ${result.total} expected files present.\n`);
    process.exit(0);
  }

  const parts = [`${result.present} present`];
  if (result.missing.length > 0) parts.push(`${result.missing.length} missing`);
  if (strict && result.extra.length > 0) parts.push(`${result.extra.length} extra`);
  process.stdout.write(`── Structure check: ${parts.join(', ')} (of ${result.total} expected)\n`);

  if (anyMissing) {
    process.exit(1);
  }
  // Extra-only without --strict → exit 0 already handled above
  // Extra-only with --strict
  if (strict && anyExtra) {
    process.exit(1);
  }
  process.exit(0);

} catch (err) {
  process.stderr.write(`× check-structure fatal: ${err.message}\n${err.stack}\n`);
  process.exit(1);
}
} // end if (require.main === module)