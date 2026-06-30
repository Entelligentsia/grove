'use strict';
// store-nlp.cjs — deterministic rule-based NLP intent parser
// No LLM, no network. Maps natural language to a traversal plan.

const { loadForgeConfig } = require('./store-facade.cjs');

const ENTITY_SYNONYMS = {
  sprints:  ['sprint', 'sprints', 'release', 'releases', 'iteration', 'iterations'],
  tasks:    ['task', 'tasks', 'item', 'items', 'work item', 'work items', 'todo', 'todos'],
  bugs:     ['bug', 'bugs', 'defect', 'defects', 'issue', 'issues', 'problem', 'problems'],
  features: ['feature', 'features', 'epic', 'epics', 'capability', 'capabilities'],
};

const STATUS_MAP = {
  'open':         { tasks: 'planned', bugs: 'in-progress', sprints: 'active', features: 'active' },
  'active':       { tasks: 'implementing', bugs: 'in-progress', sprints: 'active', features: 'active' },
  'in progress':  { tasks: 'implementing', bugs: 'in-progress', sprints: 'active', features: 'active' },
  'in-progress':  { tasks: 'implementing', bugs: 'in-progress', sprints: 'active', features: 'active' },
  'completed':    { tasks: 'committed', bugs: 'fixed', sprints: 'completed', features: 'shipped' },
  'done':         { tasks: 'committed', bugs: 'fixed', sprints: 'completed', features: 'shipped' },
  'fixed':        { bugs: 'fixed' },
  'planned':      { tasks: 'planned', sprints: 'planning' },
  'planning':     { sprints: 'planning' },
  'implementing': { tasks: 'implementing' },
  'implemented':  { tasks: 'implemented' },
  'committed':    { tasks: 'committed' },
  'draft':        { tasks: 'draft', features: 'draft' },
  'abandoned':    { tasks: 'abandoned', sprints: 'abandoned' },
  'retired':      { features: 'retired' },
  'shipped':      { features: 'shipped' },
  'triaged':      { bugs: 'triaged' },
  'reported':     { bugs: 'reported' },
  'blocked':      { tasks: 'blocked' },
  'critical':     { _field: 'severity', bugs: 'critical' },
  'major':        { _field: 'severity', bugs: 'major' },
  'minor':        { _field: 'severity', bugs: 'minor' },
};

const ALL_STOP_WORDS = new Set([
  'list','all','the','show','find','what','which','are','in','for','about','related','to',
  'of','and','with','details','status','how','many','there','a','an','is','that','this','on',
  'by','me','give','get','tell','please','can','do','does','did','was','were','been','being',
  'have','has','had','will','would','could','should','may','might',
  'blocking','blocked','block','severity','titles','title',
  ...Object.values(ENTITY_SYNONYMS).flat(),
]);

function idRegexes() {
  const p = loadForgeConfig().prefix;
  const esc = p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return {
    taskId: new RegExp(`\\b${esc}-S\\d+-T\\d+\\b`, 'i'),
    bugId: new RegExp(`\\b${esc}-BUG-\\d+\\b`, 'i'),
    taskIdAnchored: new RegExp(`^${esc}-S\\d+-T\\d+$`),
    bugIdAnchored: new RegExp(`^${esc}-BUG-\\d+$`),
  };
}

function parseIntentNLP(intent) {
  const stripped = String(intent).replace(/^\s*forge[-_ ]?store\s*:?\s*/i, '');
  const lower = stripped.toLowerCase().replace(/[^\w\s-]/g, ' ').replace(/\s+/g, ' ').trim();
  const plan = {
    traverse: {
      primary: null,
      filter: {},
      follow: [],
      keywordMatch: { field: 'title', terms: [] },
    },
  };

  const consumed = new Set();
  const _idRe = idRegexes();

  // ── Stage 1: ID patterns ──
  const idPatterns = [
    { re: _idRe.taskId,       filter: 'taskId',    entity: 'tasks' },
    { re: _idRe.bugId,        filter: 'bugId',     entity: 'bugs' },
    { re: /\bFEAT-\d+\b/i,   filter: 'featureId', entity: null },
    { re: /\bS\d+\b/i,        filter: 'sprintId',  entity: null },
    { re: /sprint\s+(\d+)/i,  filter: 'sprintId',  entity: null, format: v => 'S' + v },
  ];
  let idEntityHint = null;
  for (const { re, filter: filterKey, entity, format } of idPatterns) {
    const m = lower.match(re);
    if (m) {
      const value = format ? format(m[1]) : m[0].toUpperCase();
      plan.traverse.filter[filterKey] = value;
      if (entity) idEntityHint = entity;
      const words = lower.split(/\s+/);
      const matchText = m[0].toLowerCase();
      words.forEach((w, i) => { if (matchText.includes(w)) consumed.add(i); });
      break;
    }
  }

  // ── Stage 2: Entity detection ──
  const words = lower.split(/\s+/);
  let detectedEntity = null;
  outer:
  for (let i = 0; i < words.length; i++) {
    if (consumed.has(i)) continue;
    // "with <entity>" (and "<entity> for") are FK-follow directives handled in
    // Stage 4 — they must NOT be treated as the primary entity. Otherwise an
    // explicit anchored ID (e.g. "WI-S19-T04 with sprint with feature") loses
    // its entity to the follow-word, its filter is stripped as invalid, and the
    // query degrades into a full-store scan.
    if (i > 0 && words[i - 1] === 'with') continue;
    const w = words[i];
    if (i + 1 < words.length) {
      const bigram = w + ' ' + words[i + 1];
      for (const [entity, synonyms] of Object.entries(ENTITY_SYNONYMS)) {
        if (synonyms.includes(bigram)) {
          detectedEntity = entity;
          consumed.add(i);
          consumed.add(i + 1);
          break outer;
        }
      }
    }
    for (const [entity, synonyms] of Object.entries(ENTITY_SYNONYMS)) {
      if (synonyms.includes(w)) {
        detectedEntity = entity;
        consumed.add(i);
        break outer;
      }
    }
  }

  if (!detectedEntity && !idEntityHint && plan.traverse.filter.sprintId) {
    plan.traverse.primary = 'sprints';
  } else {
    plan.traverse.primary = detectedEntity || idEntityHint || 'tasks';
  }

  // ── Stage 3: Status/severity filters ──
  for (let i = 0; i < words.length; i++) {
    if (consumed.has(i)) continue;
    if (i + 1 < words.length) {
      const bigram = words[i] + ' ' + words[i + 1];
      if (STATUS_MAP[bigram]) {
        const mapping = STATUS_MAP[bigram];
        const field = mapping._field || 'status';
        const value = mapping[plan.traverse.primary];
        if (value) {
          plan.traverse.filter[field] = value;
          consumed.add(i);
          consumed.add(i + 1);
          continue;
        }
      }
    }
    const w = words[i];
    if (STATUS_MAP[w]) {
      const mapping = STATUS_MAP[w];
      const field = mapping._field || 'status';
      const value = mapping[plan.traverse.primary];
      if (value) {
        plan.traverse.filter[field] = value;
        consumed.add(i);
      }
    }
  }

  // ── Stage 4: FK follow phrases ──
  if (/\bwith\s+sprints?\b/.test(lower) || /\bsprint\s+for\b/.test(lower) || /\bwhich sprint\b/.test(lower)) {
    if (!plan.traverse.follow.includes('sprintId')) plan.traverse.follow.push('sprintId');
  }
  if (/\bwith\s+features?\b/.test(lower) || /\bfeature\s+for\b/.test(lower)) {
    if (!plan.traverse.follow.includes('featureId')) plan.traverse.follow.push('featureId');
  }
  if (/\bblock/i.test(lower) || /\bblocking\b/.test(lower)) {
    if (plan.traverse.primary === 'bugs' && !plan.traverse.follow.includes('blockedBy')) {
      plan.traverse.follow.push('blockedBy');
    }
    if (plan.traverse.primary === 'tasks' && !plan.traverse.follow.includes('blocksTask')) {
      plan.traverse.follow.push('blocksTask');
    }
  }

  // ── Stage 4b: Ordering / limit / count ──
  let orderDir = null;
  let limitN = null;
  let countMode = false;

  const biMap = [
    { phrase: 'most recent', dir: 'desc', limit: 1 },
    { phrase: 'how many',    count: true },
    { phrase: 'count of',    count: true },
    { phrase: 'number of',   count: true },
  ];
  for (let i = 0; i < words.length - 1; i++) {
    if (consumed.has(i) || consumed.has(i + 1)) continue;
    const bg = words[i] + ' ' + words[i + 1];
    const hit = biMap.find(b => b.phrase === bg);
    if (hit) {
      if (hit.dir) orderDir = orderDir || hit.dir;
      if (hit.limit && !limitN) limitN = hit.limit;
      if (hit.count) countMode = true;
      consumed.add(i); consumed.add(i + 1);
    }
  }

  for (let i = 0; i < words.length; i++) {
    if (consumed.has(i)) continue;
    const w = words[i];
    const next = words[i + 1];
    const isNum = next && /^\d+$/.test(next);
    if ((w === 'top' || w === 'first' || w === 'last') && isNum) {
      limitN = parseInt(next, 10);
      if (w === 'last') orderDir = orderDir || 'desc';
      else orderDir = orderDir || (w === 'top' ? 'desc' : 'asc');
      consumed.add(i); consumed.add(i + 1);
      continue;
    }
    if (w === 'latest' || w === 'newest' || w === 'recent' || w === 'last') {
      orderDir = orderDir || 'desc';
      if (!limitN) limitN = 1;
      consumed.add(i);
    } else if (w === 'oldest' || w === 'earliest' || w === 'first') {
      orderDir = orderDir || 'asc';
      if (!limitN) limitN = 1;
      consumed.add(i);
    } else if (w === 'count') {
      countMode = true;
      consumed.add(i);
    }
  }

  if (orderDir) plan.traverse.sort = orderDir;
  if (limitN)   plan.traverse.limit = limitN;
  if (countMode) plan.traverse.count = true;

  // ── Stage 5: Keyword extraction ──
  const keywords = words.filter(
    (w, i) => !consumed.has(i) && w.length > 1 && !ALL_STOP_WORDS.has(w) && !/^\d+$/.test(w)
  );
  plan.traverse.keywordMatch.terms = [...new Set(keywords)];

  return plan;
}

function extractKeywordsFromIntent(intent) {
  const stopWords = new Set([
    'list','all','the','show','find','what','which','are','in','for','about','related','to',
    'of','and','with','sprints','tasks','bugs','features','details','status','open','closed',
    'active','completed','how','many','there','a','an','is','that','this','on','by','me',
    'give','get','tell','please','can','do','does','did','was','were','been','being',
    'have','has','had','will','would','could','should','may','might',
  ]);
  return intent.toLowerCase()
    .replace(/[^a-z0-9\s-]/g, ' ')
    .split(/[\s-]+/)
    .filter(w => w.length > 2 && !stopWords.has(w));
}

module.exports = { parseIntentNLP, extractKeywordsFromIntent, ENTITY_SYNONYMS, STATUS_MAP };
