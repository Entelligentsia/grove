#!/usr/bin/env node
'use strict';

// Forge tool: verify-apply
// Verify that claimed file modifications actually landed on disk by checking
// each path against the generation-manifest baseline.
//
// Usage: node verify-apply.cjs <path1> [<path2> ...]
//
// Output: JSON to stdout:
//   { "modified": [...], "unchanged": [...], "untracked": [...], "missing": [...] }
//
// Exit codes:
//   0 = all paths verified modified (success — agent claim was correct)
//   1 = any unchanged paths present (agent claim was wrong — re-apply needed)
//   2 = usage error (no paths provided)

const { spawnSync } = require('child_process');
const path = require('path');
const { findProjectRoot } = require('./lib/project-root.cjs');

if (require.main === module) {

const claimedPaths = process.argv.slice(2);

if (claimedPaths.length === 0) {
  process.stderr.write('Usage: node verify-apply.cjs <path1> [<path2> ...]\n');
  process.exit(2);
}

const projectRoot = findProjectRoot() || process.cwd();
const toolDir = __dirname;
const manifestTool = path.join(toolDir, 'generation-manifest.cjs');

const result = {
  modified:  [],
  unchanged: [],
  untracked: [],
  missing:   [],
};

for (const claimedPath of claimedPaths) {
  const r = spawnSync('node', [manifestTool, 'check', claimedPath], {
    cwd: projectRoot,
    encoding: 'utf8',
    timeout: 5_000,
  });
  switch (r.status) {
    case 0:
      result.unchanged.push(claimedPath);
      break;
    case 1:
      result.modified.push(claimedPath);
      break;
    case 2:
      result.untracked.push(claimedPath);
      break;
    case 3:
      result.missing.push(claimedPath);
      break;
    default:
      // Spawn error, timeout, or unrecognized exit code — safety default: missing
      result.missing.push(claimedPath);
  }
}

process.stdout.write(JSON.stringify(result, null, 2) + '\n');

// Exit 1 if any unchanged paths (agent claim was wrong)
if (result.unchanged.length > 0) {
  process.exit(1);
}
process.exit(0);

} // end if (require.main === module)
