'use strict';

// Shared JSON Schema validator for Forge.
//
// Minimal Draft-07 subset used by both store-cli.cjs (tool writes) and the
// write-boundary hook (direct agent writes). Keeps Forge dependency-free.
//
// Supported keywords:
//   type (string|number|integer|boolean|array|object; union via array)
//   required, properties, additionalProperties: false
//   enum, minimum, maxLength, minLength, maxItems, items (type + maxLength)
//   pattern (ECMA regex against string values)
//   format: "date-time" (ISO 8601)
//
// Not supported (by design): $ref, allOf/anyOf/oneOf, propertyNames,
// dependencies, const. Schemas can express what Forge needs without them.

// FORGE-S22-T03: import suggestion engine for "Did you mean?" on enum/field errors
const { suggest, formatSuggestion } = require('./suggest.cjs');

// Fields that may legitimately be null (nullable FKs / optional timing).
// Mirrors store-cli.cjs — keep in sync.
const NULLABLE_FIELDS = new Set([
  'sprintId', 'taskId', 'endTimestamp', 'durationMinutes',
  'feature_id', 'description', 'completedAt', 'resolvedAt'
]);

function isDateTime(s) {
  if (typeof s !== 'string') return false;
  // Accept standard ISO 8601 / RFC 3339 date-time strings.
  // Require: YYYY-MM-DDTHH:MM:SS(.sss)?(Z|±HH:MM)
  return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,9})?(Z|[+-]\d{2}:?\d{2})$/.test(s);
}

function typeMatches(expected, val) {
  if (expected === 'integer') return Number.isInteger(val);
  if (expected === 'number')  return typeof val === 'number' && !Number.isNaN(val);
  if (expected === 'array')   return Array.isArray(val);
  if (expected === 'null')    return val === null;
  if (expected === 'object')  return val !== null && typeof val === 'object' && !Array.isArray(val);
  return typeof val === expected;
}

function validateRecord(record, schema, opts) {
  opts = opts || {};
  const errors = [];

  if (record === null || typeof record !== 'object' || Array.isArray(record)) {
    errors.push('record: expected object');
    return errors;
  }

  const required = schema.required || [];
  for (const field of required) {
    const v = record[field];
    if (v === undefined || v === '') {
      errors.push(`${field}: missing required field`);
    } else if (v === null && !NULLABLE_FIELDS.has(field)) {
      errors.push(`${field}: missing required field`);
    }
  }

  const properties = schema.properties || {};

  for (const [field, def] of Object.entries(properties)) {
    const val = record[field];
    if (val === undefined || val === null) continue;

    if (def.type) {
      const ok = Array.isArray(def.type)
        ? def.type.some(t => typeMatches(t, val))
        : typeMatches(def.type, val);
      if (!ok) {
        errors.push(`${field}: expected ${def.type}, got ${Array.isArray(val) ? 'array' : typeof val}`);
        continue;
      }
    }

    if (def.enum && !def.enum.includes(val)) {
      const enumSuggestions = suggest(String(val), def.enum);
      const suggestionStr = formatSuggestion(enumSuggestions);
      errors.push(`${field}: value "${val}" not in [${def.enum.join(', ')}]${suggestionStr ? ' ' + suggestionStr : ''}`);
    }

    if (def.minimum !== undefined && typeof val === 'number' && val < def.minimum) {
      errors.push(`${field}: value ${val} is below minimum ${def.minimum}`);
    }

    if (typeof val === 'string') {
      if (def.maxLength !== undefined && val.length > def.maxLength) {
        errors.push(`${field}: value length ${val.length} exceeds maxLength ${def.maxLength}`);
      }
      if (def.minLength !== undefined && val.length < def.minLength) {
        errors.push(`${field}: value length ${val.length} is below minLength ${def.minLength}`);
      }
      if (def.pattern) {
        let re;
        try { re = new RegExp(def.pattern); }
        catch (_) { errors.push(`${field}: invalid schema pattern`); re = null; }
        if (re && !re.test(val)) {
          errors.push(`${field}: value "${val}" does not match pattern ${def.pattern}`);
        }
      }
      if (def.format === 'date-time' && !isDateTime(val)) {
        errors.push(`${field}: value "${val}" is not a valid date-time`);
      }
    }

    if (Array.isArray(val)) {
      if (def.maxItems !== undefined && val.length > def.maxItems) {
        errors.push(`${field}: array has ${val.length} items, exceeds maxItems ${def.maxItems}`);
      }
      if (def.items) {
        val.forEach((item, idx) => {
          if (def.items.type && !typeMatches(def.items.type, item)) {
            errors.push(`${field}[${idx}]: expected ${def.items.type}, got ${typeof item}`);
          }
          if (def.items.maxLength !== undefined && typeof item === 'string' && item.length > def.items.maxLength) {
            errors.push(`${field}[${idx}]: item length ${item.length} exceeds maxLength ${def.items.maxLength}`);
          }
        });
      }
    }
  }

  if (schema.additionalProperties === false) {
    const allowed = new Set(Object.keys(properties));
    const fieldNames = Object.keys(properties);
    for (const key of Object.keys(record)) {
      if (!allowed.has(key)) {
        const fieldSuggestions = suggest(key, fieldNames);
        const suggestionStr = formatSuggestion(fieldSuggestions);
        errors.push(`${key}: undeclared field${suggestionStr ? ' ' + suggestionStr : ''}`);
      }
    }
  }

  // Append a hint when validation fails so the LLM/user knows where to look
  // for a canonical sample. The opts.entity hint is set by callers that know
  // which entity they're validating (store-cli.cjs cmdWrite, cmdValidate).
  if (errors.length > 0 && opts.entity) {
    errors.push(
      `(hint: run 'node store-cli.cjs template ${opts.entity}' for a canonical sample, ` +
      `or 'node store-cli.cjs describe ${opts.entity}' for the raw JSON Schema)`
    );
  }

  return errors;
}

module.exports = { validateRecord, isDateTime, NULLABLE_FIELDS };
