'use strict';

// Structured Result helpers for Forge CJS module APIs.
//
// Pattern: every exported function that can fail returns either
//   { ok: true, value }              — success
//   { ok: false, code, message }     — failure
//
// Code is always a string from RESULT_CODES, never a magic literal.

const RESULT_CODES = {
  // collate.cjs — resolveTaskDir
  MISSING_DIR:      'MISSING_DIR',      // no matching task directory found on disk

  // estimate-usage.cjs — estimateTokens
  E_ZERO_DURATION:    'E_ZERO_DURATION',    // durationMinutes is exactly 0
  E_MISSING_DURATION: 'E_MISSING_DURATION', // durationMinutes is null or undefined
};

/**
 * Wrap a success value in a Result.
 * @template T
 * @param {T} [value]
 * @returns {{ ok: true, value: T }}
 */
function ok(value) {
  return { ok: true, value };
}

/**
 * Wrap an error in a Result.
 * @param {string} code  — one of RESULT_CODES
 * @param {string} message — human-readable explanation
 * @returns {{ ok: false, code: string, message: string }}
 */
function fail(code, message) {
  return { ok: false, code, message };
}

module.exports = { ok, fail, RESULT_CODES };
