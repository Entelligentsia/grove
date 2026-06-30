'use strict';
// lib/fsutil.cjs — filesystem utility helpers.
//
// DECISION (FORGE-S25-T07, closes T-5):
//   Extracted from four inline mkdirSync sites across forge plugin tools:
//   - tools/build-base-pack.cjs (private ensureDir function + 6 call sites)
//   - tools/friction-emit.cjs (inline fs.mkdirSync)
//   - tools/build-context-pack.cjs (2x inline fs.mkdirSync)
//   - tools/build-manifest.cjs (inline fs.mkdirSync)
//
// API:
//   ensureDir(dirPath: string) → void
//     Creates the directory recursively if it does not exist. Idempotent.
//
//   isFile(filePath: string) → boolean
//     Returns true if the path exists and is a regular file. Never throws.
//
//   isDirectory(dirPath: string) → boolean
//     Returns true if the path exists and is a directory. Never throws.

const fs = require('node:fs');

/**
 * Create a directory (and all intermediate parents) if it does not exist.
 * Safe to call on an existing directory — no-op in that case.
 *
 * @param {string} dirPath - Absolute or relative directory path.
 */
function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

/**
 * Check whether a path exists and is a regular file.
 *
 * @param {string} filePath - Path to test.
 * @returns {boolean}
 */
function isFile(filePath) {
  try {
    return fs.statSync(filePath).isFile();
  } catch (_) {
    return false;
  }
}

/**
 * Check whether a path exists and is a directory.
 *
 * @param {string} dirPath - Path to test.
 * @returns {boolean}
 */
function isDirectory(dirPath) {
  try {
    return fs.statSync(dirPath).isDirectory();
  } catch (_) {
    return false;
  }
}

module.exports = { ensureDir, isFile, isDirectory };
