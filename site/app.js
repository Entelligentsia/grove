/* grove — site behaviour
 *
 * One job: reveal sections as they scroll in. Everything the page needs to say
 * is in the markup, so the page stays fully readable if this file never runs.
 */

// The seven structural tools, in the order an agent meets them: orient in a
// repo, find a name, read one thing, trace it, verify the edit.
const TOOLS = [
  {
    key: 'map',
    phase: 'Orient',
    name: 'grove map <dir>',
    desc: 'Cold-start orientation. A directory\'s definitions and their outgoing references — the shape of a subsystem without a single body.',
    cmd: 'grove map src/',
    output: `src/\n  ├── server.py (defs: Server, main | refs: Request, Response)\n  └── router.py (defs: Router | refs: Match)`,
  },
  {
    key: 'outline',
    phase: 'Orient',
    name: 'grove outline <file>',
    desc: 'A file\'s definition skeleton — kind, name, parent, signature, id. Structural overview without reading the file.',
    cmd: 'grove outline src/server.py',
    output: `class  Server            12:0   py:src/server.py#Server\n  def  __init__          14:4   py:src/server.py#Server.__init__\n  def  handle_request    31:4   py:src/server.py#Server.handle_request\ndef    main              88:0   py:src/server.py#main`,
  },
  {
    key: 'symbols',
    phase: 'Find',
    name: 'grove symbols <dir> --name <n>',
    desc: 'Repo-wide symbol search. Exact by default, --name-contains for substring. Every hit carries a stable id you pass forward.',
    cmd: 'grove symbols src/ --name handle_request',
    output: `[\n  {\n    "symbol_id": "py:src/server.py#Server.handle_request",\n    "name": "handle_request",\n    "kind": "function",\n    "range": { "start_line": 31, "end_line": 45 }\n  }\n]`,
  },
  {
    key: 'source',
    phase: 'Read',
    name: 'grove source <id>',
    desc: 'One symbol\'s body, by exact bytes. This is the call that replaces a whole-file read.',
    cmd: 'grove source "py:src/server.py#Server.handle_request"',
    output: `def handle_request(self, req: Request) -> Response:\n    """Processes incoming HTTP requests structurally."""\n    handler = self.router.match(req.path)\n    return handler(req)`,
  },
  {
    key: 'definition',
    phase: 'Locate',
    name: 'grove definition <name>',
    desc: 'Go-to-def by symbol name, or from a usage position with --at file:line:col. Lines and columns are 1-based.',
    cmd: 'grove definition Server',
    output: `Defined at py:src/server.py#Server (line 12, col 0)`,
  },
  {
    key: 'callers',
    phase: 'Trace',
    name: 'grove callers <name> -d <dir>',
    desc: 'Every call site of a symbol across the repo, each tied to the function that encloses it.',
    cmd: 'grove callers handle_request -d src/',
    output: `src/app.py:102:4 -> called inside main()`,
  },
  {
    key: 'check',
    phase: 'Verify',
    name: 'grove check <file>',
    desc: 'Post-edit syntax check. Tree-sitter parses error-tolerantly, so ERROR and MISSING nodes surface immediately. Exits 1 if any.',
    cmd: 'grove check src/server.py',
    output: `✓ src/server.py parsed cleanly (0 ERROR / MISSING nodes)`,
  },
];

function initToolExplorer() {
  const nav = document.getElementById('tool-nav');
  if (!nav) return;

  const fields = {
    phase: document.getElementById('tool-phase'),
    name: document.getElementById('tool-name'),
    desc: document.getElementById('tool-desc'),
    cmd: document.getElementById('tool-cmd'),
    output: document.getElementById('tool-output'),
  };

  const tabs = TOOLS.map((tool, i) => {
    const b = document.createElement('button');
    b.className = 'tool-tab';
    b.type = 'button';
    b.role = 'tab';
    b.textContent = tool.key;
    b.setAttribute('aria-selected', String(i === 0));
    b.addEventListener('click', () => select(i));
    nav.appendChild(b);
    return b;
  });

  function select(i) {
    const tool = TOOLS[i];
    tabs.forEach((t, j) => t.setAttribute('aria-selected', String(i === j)));
    fields.phase.textContent = tool.phase;
    fields.name.textContent = tool.name;
    fields.desc.textContent = tool.desc;
    fields.cmd.textContent = tool.cmd;
    fields.output.textContent = tool.output;
  }

  // Arrow keys move between tabs, as a tablist should.
  nav.addEventListener('keydown', (e) => {
    const cur = tabs.findIndex((t) => t.getAttribute('aria-selected') === 'true');
    let next = null;
    if (e.key === 'ArrowRight') next = (cur + 1) % tabs.length;
    if (e.key === 'ArrowLeft') next = (cur - 1 + tabs.length) % tabs.length;
    if (next === null) return;
    e.preventDefault();
    select(next);
    tabs[next].focus();
  });

  select(1); // open on `outline` — the tool people recognise first
}

document.addEventListener('DOMContentLoaded', () => {
  initToolExplorer();

  const reveals = document.querySelectorAll('.reveal');

  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduced || !('IntersectionObserver' in window)) {
    reveals.forEach((el) => el.classList.add('in'));
    return;
  }

  const io = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add('in');
        io.unobserve(entry.target);
      });
    },
    { rootMargin: '0px 0px -8% 0px', threshold: 0.04 }
  );

  reveals.forEach((el) => io.observe(el));

  // Reveal anything at or above the fold outright. An element scrolled *past*
  // never intersects, so without this it stays at opacity 0 for good — which
  // happens on any deep anchor, and on reload, where Chrome restores the scroll
  // position after DOMContentLoaded. Hence sweeping again on load and on the
  // first scroll rather than trusting a single pass.
  function sweep() {
    let remaining = 0;
    reveals.forEach((el) => {
      if (el.classList.contains('in')) return;
      if (el.getBoundingClientRect().top < window.innerHeight) {
        el.classList.add('in');
        io.unobserve(el);
      } else {
        remaining += 1;
      }
    });
    if (!remaining) window.removeEventListener('scroll', sweep);
  }

  sweep();
  window.addEventListener('load', sweep);
  window.addEventListener('scroll', sweep, { passive: true });
});
