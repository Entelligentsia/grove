'use strict';
// store-query-exec.cjs — query execution, result assembly, FK traversal

const path = require('path');
const { extractExcerpt, loadForgeConfig, findIndexPath } = require('./store-facade.cjs');
const { extractKeywordsFromIntent } = require('./store-nlp.cjs');

// Default safety cap for NLP listings without an explicit limit. Prevents an
// unbounded query from dumping the entire store into an LLM's context (a single
// uncapped query consumed ~28% of a plan run's input tokens — see
// doc/plans/store-query-nlp-response-shape-improvement.md). Callers that need
// everything use the exact `--sprint`/`--task` flag paths (which bypass this) or
// pass an explicit limit.
const DEFAULT_NLP_LIMIT = 25;

function escapeRe(s) {
  return String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// Word-boundary match. Prevents "store" matching "restore".
function kwMatches(text, term) {
  if (!term) return false;
  const re = new RegExp(`(?:^|[^a-z0-9])${escapeRe(term.toLowerCase())}(?:$|[^a-z0-9])`, 'i');
  return re.test(String(text || ''));
}

function sortKeyFor(entity, primary) {
  const tail = s => { const m = String(s || '').match(/(\d+)$/); return m ? parseInt(m[1], 10) : 0; };
  switch (primary) {
    case 'sprints':  return tail(entity.sprintId);
    case 'tasks':    return tail(entity.sprintId) * 10000 + tail(entity.taskId);
    case 'bugs':     return tail(entity.bugId);
    case 'features': return tail(entity.featureId || entity.feature_id);
  }
  return 0;
}

function buildResult(entity, type, store, includeExcerpts) {
  const cfg = loadForgeConfig();
  const idField = type === 'sprint' ? 'sprintId'
    : type === 'task'    ? 'taskId'
    : type === 'bug'     ? 'bugId'
    : (entity.featureId ? 'featureId' : 'id');
  const id = entity[idField] || 'unknown';

  const result = {
    id,
    title: entity.title || entity.name || '',
    status: entity.status || '',
    type,
    relationships: {},
  };

  if (entity.sprintId)                         result.relationships.sprintId  = entity.sprintId;
  if (entity.featureId || entity.feature_id)   result.relationships.featureId = entity.featureId || entity.feature_id;
  if (entity.blockedBy)                         result.relationships.blockedBy = entity.blockedBy;
  if (entity.blocksTask)                        result.relationships.blocksTask = entity.blocksTask;

  const pluralType = type === 'bug' ? 'bugs' : type === 'task' ? 'tasks' : type === 'sprint' ? 'sprints' : 'features';
  const jsonPath = path.join(cfg.storePathRel, pluralType, `${id}.json`);
  const mdPath = findIndexPath(entity, cfg.kbPath) || '';
  result.fileRefs  = { json: jsonPath, md: mdPath };
  result.storeRef  = jsonPath;
  result.indexRef  = mdPath;
  result.excerpt   = includeExcerpts ? extractExcerpt(mdPath) : null;

  return result;
}

function buildFieldValidators() {
  const { loadForgeConfig: cfg } = require('./store-facade.cjs');
  const p = cfg().prefix;
  const esc = p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return {
    sprints: {
      sprintId: /^S\d+$/,
      status: ['planning', 'active', 'completed', 'retrospective-done', 'blocked', 'partially-completed', 'abandoned'],
    },
    tasks: {
      taskId:    new RegExp(`^${esc}-S\\d+-T\\d+$`),
      sprintId:  /^S\d+$/,
      featureId: /^FEAT-\d+$/,
      status: ['draft', 'planned', 'plan-approved', 'implementing', 'implemented', 'review-approved', 'approved', 'committed', 'plan-revision-required', 'code-revision-required', 'blocked', 'escalated', 'abandoned'],
    },
    bugs: {
      bugId:    new RegExp(`^${esc}-BUG-\\d+$`),
      sprintId: /^S\d+$/,
      severity: ['critical', 'major', 'minor'],
      status:   ['reported', 'triaged', 'in-progress', 'fixed', 'verified'],
    },
    features: {
      featureId: /^FEAT-\d+$/,
      status: ['active', 'draft', 'shipped', 'retired'],
    },
  };
}

function validatePlan(plan) {
  const validators = buildFieldValidators();
  const warnings = [];
  let confidence = 'high';
  const traverse = plan.traverse || plan;
  const singMap = { bug: 'bugs', task: 'tasks', sprint: 'sprints', feature: 'features' };
  const primary = singMap[traverse.primary] || traverse.primary;
  const valid = new Set(['tasks', 'bugs', 'sprints', 'features', ...Object.keys(singMap)]);

  if (!valid.has(primary)) {
    warnings.push(`invalid primary "${traverse.primary}"`);
    confidence = 'low';
  }

  const fields = validators[primary];
  const filter = traverse.filter || {};
  if (fields) {
    for (const [key, val] of Object.entries(filter)) {
      if (!fields[key]) {
        warnings.push(`invalid filter key "${key}" for ${primary}`);
        confidence = 'low';
      } else if (fields[key] instanceof RegExp) {
        if (!fields[key].test(String(val))) {
          warnings.push(`filter ${key}="${val}" does not match pattern`);
          confidence = 'low';
        }
      } else if (Array.isArray(fields[key])) {
        if (!fields[key].includes(String(val))) {
          warnings.push(`filter ${key}="${val}" not in enum`);
          confidence = 'low';
        }
      }
    }
  }
  return { confidence, warnings };
}

function executeQuery(plan, store, cfg) {
  const trace = [];
  const traverse = plan.traverse || plan;
  const singMap = { bug: 'bugs', task: 'tasks', sprint: 'sprints', feature: 'features' };
  let primary = singMap[traverse.primary] || traverse.primary || 'tasks';
  let filter = { ...(traverse.filter || {}) };
  const follow = traverse.follow || [];
  const kwMatch = traverse.keywordMatch || {};
  const includeExcerpts = cfg.noExcerpts !== true;

  trace.push('intent parsed via NLP rules');

  // Validate + strip invalid filters
  const validation = validatePlan(plan);
  const stripped = {};
  if (validation.warnings.length > 0) {
    const validators = buildFieldValidators();
    const fields = validators[primary];
    if (fields) {
      for (const key of Object.keys(filter)) {
        if (!fields[key]) {
          trace.push(`stripped invalid filter key: ${key}`);
          stripped[key] = filter[key];
          delete filter[key];
        } else {
          const spec = fields[key];
          if (spec instanceof RegExp && !spec.test(String(filter[key]))) {
            trace.push(`stripped filter ${key}="${filter[key]}" (value mismatch)`);
            stripped[key] = filter[key];
            delete filter[key];
          } else if (Array.isArray(spec) && !spec.includes(String(filter[key]))) {
            trace.push(`stripped filter ${key}="${filter[key]}" (not in enum)`);
            stripped[key] = filter[key];
            delete filter[key];
          }
        }
      }
    }
  }

  // Re-route on a stripped record-anchoring ID: an explicit taskId/bugId that
  // was stripped because `primary` disagreed means the caller named a specific
  // record. Honour it (re-route primary + restore the filter) instead of
  // degrading into a full-collection scan. This is the engine-layer floor that
  // backs up the parser fix in store-nlp.cjs.
  const ID_ROUTE = { taskId: 'tasks', bugId: 'bugs' };
  for (const idKey of Object.keys(ID_ROUTE)) {
    if (stripped[idKey] !== undefined && primary !== ID_ROUTE[idKey]) {
      trace.push(`re-routed primary ${primary} → ${ID_ROUTE[idKey]} (anchored ${idKey}="${stripped[idKey]}")`);
      primary = ID_ROUTE[idKey];
      filter[idKey] = stripped[idKey];
      break;
    }
  }

  // List primary entities
  let entities;
  switch (primary) {
    case 'sprints':  entities = store.listSprints(filter);  break;
    case 'tasks':    entities = store.listTasks(filter);    break;
    case 'bugs':     entities = store.listBugs(filter);     break;
    case 'features': entities = store.listFeatures(filter); break;
    default:         entities = [];
  }

  // Keyword filter
  const hasMeaningfulKw = kwMatch.field && kwMatch.terms && kwMatch.terms.length > 0
    && !kwMatch.terms.some(t => t.length > 20);
  if (hasMeaningfulKw) {
    const before = entities.length;
    entities = entities.filter(e => kwMatch.terms.some(t => kwMatches(e[kwMatch.field] || '', t)));
    trace.push(`keyword matched ${kwMatch.terms.join(', ')} on ${kwMatch.field}: ${before} → ${entities.length}`);
  } else if (Object.keys(filter).length > 0) {
    trace.push(`listed ${primary} with filter ${JSON.stringify(filter)}: ${entities.length} results`);
  } else {
    trace.push(`listed ${primary}: ${entities.length} results`);
  }

  const totalMatched = entities.length;

  // Sort
  if (traverse.sort) {
    entities.sort((a, b) => {
      const ka = sortKeyFor(a, primary);
      const kb = sortKeyFor(b, primary);
      return traverse.sort === 'desc' ? kb - ka : ka - kb;
    });
    trace.push(`sorted ${primary} ${traverse.sort}`);
  }

  // Count mode
  if (traverse.count) {
    trace.push(`count mode: ${totalMatched}`);
    return { query: cfg.query || '', path: 'intent-nlp', traversalTrace: trace, count: totalMatched, results: [], relatedFileRefs: [], totalMatched };
  }

  // Limit — honour an explicit limit; otherwise apply a default safety cap so an
  // unbounded NLP listing never dumps the whole collection into the model's
  // context. `truncated` + `totalMatched` let the caller detect and, if needed,
  // re-query with an explicit limit or the exact-flag path.
  const effectiveLimit = traverse.limit || DEFAULT_NLP_LIMIT;
  let truncated = false;
  if (entities.length > effectiveLimit) {
    trace.push(`limited to ${effectiveLimit} (of ${totalMatched})${traverse.limit ? '' : ' [default cap]'}`);
    entities = entities.slice(0, effectiveLimit);
    truncated = true;
  }

  // Build results + follow FKs
  const allResults = [];
  const relatedFiles = [];
  const singType = primary.replace(/s$/, '');

  for (const entity of entities) {
    allResults.push(buildResult(entity, singType, store, includeExcerpts));
    for (const fk of follow) {
      const related = store.followFK(entity, fk);
      if (related) {
        const relatedList = Array.isArray(related) ? related : [related];
        const fkSingType = fk.replace(/Id$/, '').replace(/s$/, '');
        for (const r of relatedList) {
          allResults.push(buildResult(r, fkSingType, store, includeExcerpts));
        }
        trace.push(`followed ${fk} → ${relatedList.length} related`);
      }
    }
  }

  for (const r of allResults) {
    if (r.fileRefs?.md) relatedFiles.push(r.fileRefs.md);
    if (r.fileRefs?.json) relatedFiles.push(r.fileRefs.json);
  }

  trace.push(`plan confidence: ${validation.confidence}`);

  // Auto-retry on zero results
  if (allResults.length === 0 && (Object.keys(filter).length > 0 || hasMeaningfulKw)) {
    trace.push('0 results — retrying with title-only keyword search');
    const retryTerms = extractKeywordsFromIntent(cfg.query || '');
    if (retryTerms.length > 0) {
      let retryEntities;
      switch (primary) {
        case 'sprints':  retryEntities = store.listSprints({});  break;
        case 'tasks':    retryEntities = store.listTasks({});    break;
        case 'bugs':     retryEntities = store.listBugs({});     break;
        case 'features': retryEntities = store.listFeatures({}); break;
        default:         retryEntities = [];
      }
      retryEntities = retryEntities.filter(e => retryTerms.some(t => kwMatches(e.title || '', t)));
      trace.push(`retry: keyword "${retryTerms.join(', ')}" in ${primary}: ${retryEntities.length} results`);
      for (const entity of retryEntities) {
        allResults.push(buildResult(entity, singType, store, includeExcerpts));
      }
      if (allResults.length > 0) trace.push('overall confidence: low (required retry)');
    }
  }

  return {
    query:          cfg.query || '',
    path:           'intent-nlp',
    traversalTrace: trace,
    results:        allResults,
    totalMatched,
    returned:       allResults.length,
    truncated,
    limit:          traverse.limit || null,
    sort:           traverse.sort || null,
    relatedFileRefs: [...new Set(relatedFiles)],
  };
}

module.exports = { executeQuery, buildResult, kwMatches };
