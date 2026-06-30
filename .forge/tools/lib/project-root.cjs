'use strict';

const fs = require('fs');
const path = require('path');

/**
 * Walk up from startDir to find the project root (directory containing .forge/config.json).
 * @param {string} [startDir] - Directory to start searching from. Defaults to process.cwd().
 * @returns {string|null} Absolute path to the project root, or null if not found.
 */
function findProjectRoot(startDir) {
  let dir = path.resolve(startDir || process.cwd());
  const root = path.parse(dir).root;

  while (dir !== root) {
    const configPath = path.join(dir, '.forge', 'config.json');
    if (fs.existsSync(configPath)) {
      return dir;
    }
    dir = path.dirname(dir);
  }

  // Check root directory as well
  const rootConfigPath = path.join(root, '.forge', 'config.json');
  if (fs.existsSync(rootConfigPath)) {
    return root;
  }

  return null;
}

module.exports = { findProjectRoot };