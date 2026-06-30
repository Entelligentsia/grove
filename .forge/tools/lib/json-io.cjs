'use strict';
// lib/json-io.cjs — shared JSON read/write primitives.
//
// Extracted from FSImpl._readJson / FSImpl._writeJson in tools/store.cjs so
// any tool or hook can reuse them without taking a dependency on the full
// store facade.

const fs = require('fs');
const path = require('path');

/**
 * Read and parse a JSON file.
 *
 * @param {string} filePath - absolute or cwd-relative path to the JSON file
 * @returns {object|null} parsed object, or null if the file does not exist
 * @throws {SyntaxError} if the file exists but contains invalid JSON (fail-fast)
 */
function readJson(filePath) {
  if (!fs.existsSync(filePath)) return null;
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

/**
 * Write an object to a JSON file with 2-space indentation and a trailing
 * newline. Creates all parent directories as needed.
 *
 * Mirrors FSImpl._writeJson contract exactly so consumers can swap in this
 * function without behavioural change.
 *
 * @param {string} filePath - absolute or cwd-relative path for the output file
 * @param {object} data     - value to serialise
 * @returns {object} the same `data` object that was written
 */
function writeJson(filePath, data) {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n', 'utf8');
  return data;
}

module.exports = { readJson, writeJson };
