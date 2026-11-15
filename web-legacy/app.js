/* Flightstrip replay.
 *
 * Reads a session.json (the wire format from flightstrip-core) and draws it as a
 * paper strip chart: prompts as bands, steps as marks on three lanes, incidents
 * as red spikes. No framework, no bundler — the whole client is this file.
 *
 * The chart is SVG built once per session; playback only flips opacity on
 * elements that already exist, so scrubbing stays smooth on a long session.
 */
'use strict';

const SVG_NS = 'http://www.w3.org/2000/svg';

const state = {
  sessions: [],
  session: null,
  t: 0,
  playing: false,
  speed: 8,
  selected: null,
  marks: [],
  x: null,
  maxT: 1,
  frame: null,
};

const dom = {
  select: document.getElementById('session-select'),
  open: document.getElementById('load-file'),
  file: document.getElementById('file-input'),
  title: document.getElementById('session-title'),
  meta: document.getElementById('session-meta'),
  verdict: document.getElementById('verdict'),
  play: document.getElementById('play'),
  playIcon: document.getElementById('play-icon'),
  restart: document.getElementById('restart'),
  scrub: document.getElementById('scrub'),
  clock: document.getElementById('clock'),
  clockTotal: document.getElementById('clock-total'),
  tape: document.getElementById('tape'),
  legend: document.getElementById('tape-legend'),
  steps: document.getElementById('steps'),
  detail: document.getElementById('detail'),
  files: document.getElementById('files-body'),
  filesHint: document.getElementById('files-hint'),
};

/* ── small helpers ──────────────────────────────────────────────────────── */

function svg(tag, attrs = {}, text) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value !== null && value !== undefined) node.setAttribute(key, String(value));
  }
  if (text !== undefined) node.textContent = text;
  return node;
}

function esc(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function clock(ms) {
  const total = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

function duration(ms) {
  const total = Math.round(ms / 1000);
  if (total >= 3600) return `${Math.floor(total / 3600)}h ${Math.floor((total % 3600) / 60)}m`;
  if (total >= 60) return `${Math.floor(total / 60)}m ${String(total % 60).padStart(2, '0')}s`;
  return `${total}s`;
}

function pl(n, single, plural) {
  return `${n} ${n === 1 ? single : plural}`;
}

function shortestPrompt(text, max = 52) {
  const flat = String(text).replace(/\s+/g, ' ').trim();
  return flat.length <= max ? flat : `${flat.slice(0, max - 1)}…`;
}

const KIND_LANE = {
  file_change: 'file',
  command: 'command',
  test_run: 'test',
  note: 'note',
};

const LANE = {
  prompt: 26,
  note: 66,
  test: 92,
  command: 116,
  file: 140,
  axis: 160,
};

/* ── boot ───────────────────────────────────────────────────────────────── */

async function boot() {
  wireControls();
  try {
    const catalogue = await fetch('sessions.json').then((response) => {
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      return response.json();
    });
    state.sessions = catalogue.sessions || [];
  } catch (error) {
    dom.title.textContent = 'No sessions could be loaded';
    dom.meta.innerHTML = `The page could not read <code>sessions.json</code> (${esc(
      error.message,
    )}). Serve the repository instead of opening the file directly:<br /><code>node web/serve.js</code>`;
    return;
  }

  for (const [index, entry] of state.sessions.entries()) {
    const option = document.createElement('option');
    option.value = String(index);
    option.textContent = entry.name;
    dom.select.append(option);
  }

  dom.select.addEventListener('change', () => {
    const entry = state.sessions[Number(dom.select.value)];
    if (entry) load(entry.file, entry.name);
  });

  if (state.sessions.length > 0) {
    const first = state.sessions[0];
    load(first.file, first.name);
  } else {
    dom.title.textContent = 'No sessions recorded yet';
    dom.meta.textContent = 'Record one with `flightstrip watch`, export it, then list it in web/sessions.json.';
  }
}

async function load(file, name) {
  try {
    const session = await fetch(file).then((response) => {
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      return response.json();
    });
    adopt(session, name);
  } catch (error) {
    dom.title.textContent = `Could not load ${name || file}`;
    dom.meta.textContent = String(error.message || error);
  }
}

function adopt(session, name) {
  if (!session || !Array.isArray(session.steps) || !session.meta) {
    dom.title.textContent = 'That file is not a Flightstrip session';
    dom.meta.textContent = 'Expected an object with `meta`, `steps` and `stats`.';
    return;
  }
  state.session = session;
  state.t = 0;
  state.playing = false;
  state.selected = null;
  state.maxT = Math.max(1, Number(session.meta.duration_ms) || lastTime(session));
  if (name) dom.title.dataset.label = name;

  renderHeader();
  buildTape();
  buildSteps();
  renderFiles();
  renderDetail(null);
  renderFrame();
  updatePlayIcon();
}

function lastTime(session) {
  return session.steps.reduce((max, step) => Math.max(max, Number(step.t_ms) || 0), 1);
}

/* ── header ─────────────────────────────────────────────────────────────── */

function renderHeader() {
  const session = state.session;
  const meta = session.meta;
  dom.title.textContent = meta.title || 'Untitled session';

  const parts = [
    `<b>${esc(duration(Number(meta.duration_ms) || lastTime(session)))}</b> recorded`,
    `${pl(session.prompts.length, 'prompt', 'prompts')}`,
    `${pl(session.steps.length, 'step', 'steps')}`,
    `${pl(session.stats.files_touched, 'file', 'files')}`,
    `${pl(session.stats.test_runs, 'test run', 'test runs')}`,
    `on <b>${esc(meta.os)}</b>`,
    `agent <b>${esc(meta.agent || 'unspecified')}</b>`,
    `repo <b>${esc(meta.repo_name || '—')}</b>`,
  ];
  if (meta.started_at) parts.push(`started ${esc(meta.started_at.replace('T', ' ').replace('.000Z', 'Z'))}`);
  dom.meta.innerHTML = parts.join(' · ');

  const counts = { critical: 0, warning: 0, info: 0 };
  for (const incident of session.incidents) {
    if (incident.severity === 'critical') counts.critical += 1;
    else if (incident.severity === 'warn' || incident.severity === 'warning') counts.warning += 1;
    else counts.info += 1;
  }

  dom.verdict.innerHTML = '';
  if (session.incidents.length === 0) {
    const item = document.createElement('li');
    item.className = 'sev-clean';
    item.innerHTML = `<span class="dot"></span><span class="count">Clean run</span> nothing matched a rule — which is not the same as safe`;
    dom.verdict.append(item);
    return;
  }
  for (const [severity, count] of Object.entries(counts)) {
    if (count === 0) continue;
    const item = document.createElement('li');
    item.className = `sev-${severity}`;
    item.innerHTML = `<span class="dot"></span><span class="count">${count}</span> ${severity}`;
    dom.verdict.append(item);
  }
  const kinds = Object.entries(session.stats.incidents_by_kind || {})
    .map(([kind, count]) => `${kind} ${count}`)
    .join(' · ');
  if (kinds) {
    const item = document.createElement('li');
    item.style.borderStyle = 'dashed';
    item.textContent = kinds;
    dom.verdict.append(item);
  }
}

/* ── the tape ───────────────────────────────────────────────────────────── */

function buildTape() {
  const session = state.session;
  state.marks = [];
  dom.tape.innerHTML = '';

  const width = Math.max(660, Math.round(dom.tape.clientWidth || 900));
  const height = 196;
  const left = 62;
  const right = 20;
  const x0 = left;
  const x1 = width - right;
  const maxT = state.maxT;
  const x = (t) => x0 + (Math.max(0, Math.min(maxT, t)) / maxT) * (x1 - x0);
  state.x = x;

  const root = svg('svg', {
    viewBox: `0 0 ${width} ${height}`,
    width,
    height,
    role: 'img',
    'aria-label': `Strip chart: ${esc(session.meta.title)}`,
  });

  // Paper and the visible plot border.
  root.append(
    svg('rect', { x: 0.5, y: 0.5, width: width - 1, height: height - 1, fill: '#f7f2e6', stroke: '#c9bda4' }),
  );

  // Prompt bands, alternating so consecutive prompts are distinguishable.
  const prompts = session.prompts.slice().sort((a, b) => a.t_ms - b.t_ms);
  prompts.forEach((prompt, index) => {
    const start = Number(prompt.t_ms) || 0;
    const end = index + 1 < prompts.length ? Number(prompts[index + 1].t_ms) : maxT;
    const bandX = x(start);
    const bandW = Math.max(2, x(end) - bandX);
    root.append(
      svg('rect', {
        x: bandX,
        y: 8,
        width: bandW,
        height: 30,
        fill: index % 2 === 0 ? 'rgba(164,112,15,0.10)' : 'rgba(30,26,22,0.045)',
      }),
    );
    root.append(
      svg('line', { x1: bandX, y1: 8, x2: bandX, y2: 38, stroke: '#a4700f', 'stroke-width': 0.75, 'stroke-dasharray': '2 2' }),
    );
    if (bandW > 46) {
      const label = svg('text', { x: bandX + 4, y: 25, class: 'prompt-label' }, shortestPrompt(prompt.text, Math.floor(bandW / 5.2)));
      label.append(svg('title', {}, prompt.text));
      root.append(label);
    }
  });

  // Lane rules.
  for (const [lane, y] of Object.entries(LANE)) {
    if (lane === 'prompt') continue;
    root.append(
      svg('line', {
        x1: x0,
        y1: y,
        x2: x1,
        y2: y,
        stroke: lane === 'axis' ? '#c9bda4' : 'rgba(201,189,164,0.55)',
        'stroke-width': 0.75,
      }),
    );
  }
  for (const lane of ['note', 'test', 'command', 'file']) {
    root.append(svg('text', { x: 8, y: LANE[lane] + 3, class: 'lane-label' }, lane.toUpperCase()));
  }

  // Time axis: pick an interval that gives six to ten labels.
  const ticks = niceTicks(maxT);
  for (const value of ticks) {
    const tickX = x(value);
    root.append(
      svg('line', { x1: tickX, y1: LANE.axis, x2: tickX, y2: LANE.axis + 3, stroke: '#c9bda4', 'stroke-width': 0.75 }),
    );
    root.append(svg('text', { x: tickX, y: LANE.axis + 14, class: 'axis-label', 'text-anchor': 'middle' }, clock(value)));
  }

  // Steps.
  for (const [index, step] of session.steps.entries()) {
    const lane = KIND_LANE[step.kind];
    const time = Number(step.t_ms) || 0;
    if (!lane) {
      // session_start / session_end: a small triangle on the axis.
      const marker = svg('path', {
        d: `M ${x(time)} ${LANE.axis - 4} l 4 6 h -8 z`,
        fill: '#7c7266',
      });
      root.append(marker);
      state.marks.push({ t: time, node: marker });
      continue;
    }

    const group = svg('g', { class: 'step-mark', tabindex: 0, role: 'button', 'aria-label': `${step.kind} at ${clock(time)}` });
    const colour =
      step.kind === 'test_run'
        ? step.ok
          ? '#2f6a4a'
          : '#b23a2b'
        : step.kind === 'command'
          ? '#2a5b85'
          : step.kind === 'note'
            ? '#7c7266'
            : '#1e1a16';
    const radius = step.kind === 'test_run' ? 3.2 : step.kind === 'note' ? 2 : 2.6;
    group.append(
      svg('line', { x1: x(time), y1: LANE[lane] - 9, x2: x(time), y2: LANE[lane], stroke: colour, 'stroke-width': 1, 'stroke-opacity': 0.7 }),
    );
    group.append(svg('circle', { cx: x(time), cy: LANE[lane], r: radius, fill: colour }));
    group.append(svg('title', {}, describe(step)));
    group.addEventListener('click', () => selectStep(index));
    group.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        selectStep(index);
      }
    });
    root.append(group);
    state.marks.push({ t: time, node: group });
  }

  // Incidents: a spike, because they are the reason the chart exists.
  for (const incident of session.incidents) {
    const time = Number(incident.t_ms) || 0;
    const colour = incident.severity === 'critical' ? '#b23a2b' : incident.severity === 'info' ? '#2a5b85' : '#a4700f';
    const group = svg('g', { style: 'cursor:pointer' });
    group.append(
      svg('line', {
        x1: x(time),
        y1: LANE.note - 8,
        x2: x(time),
        y2: LANE.file + 8,
        stroke: colour,
        'stroke-width': 1.25,
        'stroke-opacity': 0.85,
      }),
    );
    group.append(svg('path', { d: `M ${x(time)} ${LANE.note - 16} l 5 8 h -10 z`, fill: colour }));
    group.append(svg('title', {}, `${incident.severity}: ${incident.title}`));
    group.addEventListener('click', () => selectStep(Number(incident.step)));
    root.append(group);
    state.marks.push({ t: time, node: group, always: false });
  }

  // Playhead.
  const playhead = svg('g', { class: 'playhead' });
  const line = svg('line', { x1: 0, y1: 4, x2: 0, y2: LANE.axis + 4, class: 'playhead-line' });
  const cap = svg('path', { d: 'M -4 0 l 8 0 l -4 6 z', class: 'playhead-cap' });
  playhead.append(line, cap);
  playhead.setAttribute('transform', `translate(${x0},0)`);
  root.append(playhead);
  state.playhead = playhead;
  state.playheadTime = x;

  dom.tape.append(root);
  buildLegend();
}

function buildLegend() {
  const entries = [
    ['#a4700f', 'prompt'],
    ['#1e1a16', 'file change'],
    ['#2a5b85', 'command'],
    ['#2f6a4a', 'tests passing'],
    ['#b23a2b', 'incident / failing tests'],
  ];
  dom.legend.innerHTML = entries
    .map(([colour, label]) => `<span><i class="swatch" style="background:${colour}"></i>${esc(label)}</span>`)
    .join('');
}

function niceTicks(maxT) {
  const candidates = [1000, 2000, 5000, 10000, 15000, 30000, 60000, 120000, 300000, 600000, 1800000];
  const step = candidates.find((value) => maxT / value <= 9) || candidates[candidates.length - 1];
  const ticks = [];
  for (let value = 0; value <= maxT; value += step) ticks.push(value);
  return ticks;
}

/* ── frame rendering ────────────────────────────────────────────────────── */

function renderFrame() {
  const session = state.session;
  if (!session) return;

  const t = state.t;
  for (const mark of state.marks) {
    const passed = mark.t <= t + 0.5;
    mark.node.style.opacity = passed ? '' : '0.16';
  }

  if (state.playhead) {
    state.playhead.setAttribute('transform', `translate(${state.playheadTime(t)},0)`);
  }

  dom.clock.textContent = clock(t);
  dom.clockTotal.textContent = `/ ${clock(state.maxT)}`;
  dom.scrub.value = String(Math.round((t / state.maxT) * 1000));

  const current = currentStepIndex(t);
  const rows = dom.steps.querySelectorAll('.step');
  rows.forEach((row, index) => {
    const step = session.steps[index];
    const future = (Number(step.t_ms) || 0) > t + 0.5;
    row.classList.toggle('is-future', future);
    row.classList.toggle('is-current', index === current);
  });
  if (state.playing && current >= 0) {
    const row = rows[current];
    if (row) row.scrollIntoView({ block: 'nearest' });
  }
}

function currentStepIndex(t) {
  const steps = state.session.steps;
  let index = -1;
  for (const [position, step] of steps.entries()) {
    if ((Number(step.t_ms) || 0) <= t + 0.5) index = position;
    else break;
  }
  return index;
}

/* ── playback ───────────────────────────────────────────────────────────── */

function tick(now) {
  if (!state.playing) {
    state.frame = null;
    return;
  }
  const previous = state.lastFrame ?? now;
  const delta = Math.max(0, now - previous);
  state.lastFrame = now;
  state.t = Math.min(state.maxT, state.t + delta * state.speed);
  renderFrame();
  if (state.t >= state.maxT) {
    state.playing = false;
    updatePlayIcon();
    state.frame = null;
    return;
  }
  state.frame = window.requestAnimationFrame(tick);
}

function play() {
  if (!state.session) return;
  if (state.t >= state.maxT) state.t = 0;
  state.playing = true;
  state.lastFrame = null;
  updatePlayIcon();
  if (state.frame === null) state.frame = window.requestAnimationFrame(tick);
}

function pause() {
  state.playing = false;
  updatePlayIcon();
}

function updatePlayIcon() {
  dom.playIcon.textContent = state.playing ? '❚❚' : '▶';
  dom.play.setAttribute('aria-pressed', String(state.playing));
}

function wireControls() {
  dom.play.addEventListener('click', () => {
    if (state.playing) pause();
    else play();
  });

  dom.restart.addEventListener('click', () => {
    state.t = 0;
    state.playing = false;
    updatePlayIcon();
    renderFrame();
  });

  dom.scrub.addEventListener('input', () => {
    state.playing = false;
    updatePlayIcon();
    state.t = (Number(dom.scrub.value) / 1000) * state.maxT;
    renderFrame();
  });

  for (const button of document.querySelectorAll('.speed')) {
    button.addEventListener('click', () => {
      state.speed = Number(button.dataset.speed) || 8;
      for (const other of document.querySelectorAll('.speed')) {
        other.classList.toggle('is-active', other === button);
      }
    });
  }

  dom.open.addEventListener('click', () => dom.file.click());
  dom.file.addEventListener('change', async () => {
    const file = dom.file.files && dom.file.files[0];
    if (!file) return;
    try {
      const text = await file.text();
      adopt(JSON.parse(text), file.name);
      dom.title.dataset.label = file.name;
    } catch (error) {
      dom.title.textContent = 'That file could not be read';
      dom.meta.textContent = String(error.message || error);
    } finally {
      dom.file.value = '';
    }
  });

  document.addEventListener('keydown', (event) => {
    const tag = (event.target && event.target.tagName) || '';
    if (['INPUT', 'SELECT', 'TEXTAREA'].includes(tag)) return;
    if (event.key === ' ' && tag !== 'BUTTON') {
      event.preventDefault();
      if (state.playing) pause();
      else play();
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      moveSelection(1);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      moveSelection(-1);
    }
  });

  window.addEventListener('resize', () => {
    if (!state.session) return;
    const selected = state.selected;
    buildTape();
    renderFrame();
    if (selected !== null) selectStep(selected, { keepTime: true });
  });
}

function moveSelection(delta) {
  const steps = state.session ? state.session.steps : [];
  if (steps.length === 0) return;
  const base = state.selected === null ? currentStepIndex(state.t) : state.selected;
  const next = Math.max(0, Math.min(steps.length - 1, base + delta));
  selectStep(next);
}

/* ── steps list ─────────────────────────────────────────────────────────── */

function describe(step) {
  switch (step.kind) {
    case 'file_change':
      return `${step.op} ${step.path} (+${step.lines_added} −${step.lines_removed})`;
    case 'command':
      return `${step.cmd} → ${step.exit_code === null || step.exit_code === undefined ? '?' : step.exit_code}`;
    case 'test_run':
      return `${step.cmd}: ${step.passed} passed, ${step.failed} failed, ${step.skipped} skipped`;
    case 'prompt':
      return step.text;
    case 'note':
      return step.text;
    case 'session_start':
      return `Recording started: ${step.title}`;
    case 'session_end':
      return 'Recording ended';
    default:
      return step.kind;
  }
}

function chipClass(step) {
  switch (step.kind) {
    case 'file_change':
      return 'file';
    case 'command':
      return 'command';
    case 'test_run':
      return step.ok ? 'test-ok' : 'test-bad';
    case 'prompt':
      return 'prompt';
    case 'note':
      return 'note';
    default:
      return 'life';
  }
}

function chipText(step) {
  switch (step.kind) {
    case 'file_change':
      return 'file';
    case 'command':
      return 'command';
    case 'test_run':
      return step.ok ? 'tests ok' : 'tests fail';
    case 'prompt':
      return 'prompt';
    case 'note':
      return 'note';
    case 'session_start':
      return 'start';
    case 'session_end':
      return 'end';
    default:
      return step.kind;
  }
}

function bodyHtml(step) {
  switch (step.kind) {
    case 'file_change':
      return `<code>${esc(step.path)}</code> <span class="delta"><span class="add">+${step.lines_added}</span> <span class="rem">−${step.lines_removed}</span> · ${esc(step.op)}</span>`;
    case 'command':
      return `<code>${esc(step.cmd)}</code> <span class="delta">exit ${step.exit_code === null || step.exit_code === undefined ? '?' : esc(step.exit_code)}${step.duration_ms ? ` · ${esc(step.duration_ms)}ms` : ''}</span>`;
    case 'test_run':
      return `<code>${esc(step.cmd)}</code> <span class="delta">${step.passed} passed · ${step.failed} failed · ${step.skipped} skipped</span>`;
    case 'prompt':
      return `“${esc(step.text)}”`;
    case 'note':
      return esc(step.text);
    case 'session_start':
      return esc(step.title || 'Recording started');
    case 'session_end':
      return 'Recording ended';
    default:
      return esc(step.kind);
  }
}

function buildSteps() {
  const session = state.session;
  dom.steps.innerHTML = '';

  for (const [index, step] of session.steps.entries()) {
    const item = document.createElement('li');
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'step';
    button.dataset.index = String(index);

    const incidents = session.incidents.filter((incident) => Number(incident.step) === index);
    const badges = incidents
      .map(
        (incident) =>
          `<span class="badge ${incident.severity === 'critical' ? '' : incident.severity === 'info' ? 'info' : 'warn'}">${esc(
            incident.severity,
          )}</span>`,
      )
      .join('');

    button.innerHTML = `
      <span class="step-time">${esc(clock(Number(step.t_ms) || 0))}</span>
      <span class="chip ${chipClass(step)}">${esc(chipText(step))}</span>
      <span class="step-body">${bodyHtml(step)}</span>
      <span class="badges">${badges}</span>
    `;
    button.addEventListener('click', () => selectStep(index));
    item.append(button);
    dom.steps.append(item);
  }
}

/* ── detail ─────────────────────────────────────────────────────────────── */

function selectStep(index, options = {}) {
  const session = state.session;
  if (!session) return;
  const step = session.steps[index];
  if (!step) return;

  state.selected = index;
  if (!options.keepTime) {
    state.t = Math.max(state.t, Number(step.t_ms) || 0);
    pause();
    renderFrame();
  }

  const rows = dom.steps.querySelectorAll('.step');
  rows.forEach((row, position) => {
    row.classList.toggle('is-current', position === index);
  });
  if (rows[index] && !options.keepTime) rows[index].scrollIntoView({ block: 'nearest' });

  renderDetail(index);
}

function renderDetail(index) {
  const session = state.session;
  if (index === null || !session.steps[index]) {
    dom.detail.innerHTML = '<p class="detail-empty">Pick a step on the tape or in the list to see its diff.</p>';
    return;
  }

  const step = session.steps[index];
  const incidents = session.incidents.filter((incident) => Number(incident.step) === index);
  const parts = [];

  parts.push(`
    <div class="detail-head">
      <h4>${esc(chipText(step))}</h4>
      <span class="detail-time">${esc(clock(Number(step.t_ms) || 0))} · step ${index + 1}/${session.steps.length}</span>
    </div>
  `);

  if (step.kind === 'file_change') {
    parts.push(`<p class="path">${esc(step.path)}</p>`);
    parts.push(`
      <dl class="kv">
        <dt>operation</dt><dd>${esc(step.op)}</dd>
        <dt>lines</dt><dd><span class="add">+${esc(step.lines_added)}</span> / <span class="rem">−${esc(step.lines_removed)}</span></dd>
        <dt>prompt</dt><dd>${esc(step.prompt_id || 'unattributed')}</dd>
        <dt>before</dt><dd>${esc(step.before_hash ? step.before_hash.slice(0, 12) : '—')}</dd>
        <dt>after</dt><dd>${esc(step.after_hash ? step.after_hash.slice(0, 12) : '—')}</dd>
      </dl>
    `);
  } else if (step.kind === 'command') {
    parts.push(`<p><code>${esc(step.cmd)}</code></p>`);
    parts.push(`
      <dl class="kv">
        <dt>exit</dt><dd>${step.exit_code === null || step.exit_code === undefined ? 'unknown' : esc(step.exit_code)}</dd>
        <dt>took</dt><dd>${step.duration_ms === null || step.duration_ms === undefined ? '—' : `${esc(step.duration_ms)}ms`}</dd>
        <dt>prompt</dt><dd>${esc(step.prompt_id || 'unattributed')}</dd>
      </dl>
    `);
  } else if (step.kind === 'test_run') {
    parts.push(`<p><code>${esc(step.cmd)}</code></p>`);
    parts.push(`
      <dl class="kv">
        <dt>result</dt><dd>${step.ok ? 'passing' : 'failing'}</dd>
        <dt>passed</dt><dd>${esc(step.passed)}</dd>
        <dt>failed</dt><dd>${esc(step.failed)}</dd>
        <dt>skipped</dt><dd>${esc(step.skipped)}</dd>
        <dt>total</dt><dd>${esc(step.total)}</dd>
        <dt>prompt</dt><dd>${esc(step.prompt_id || 'unattributed')}</dd>
      </dl>
    `);
  } else if (step.kind === 'prompt') {
    parts.push(`<p>“${esc(step.text)}”</p>`);
    parts.push(`<dl class="kv"><dt>id</dt><dd>${esc(step.id)}</dd><dt>source</dt><dd>${esc(step.source)}</dd></dl>`);
  } else if (step.kind === 'note') {
    parts.push(`<p>${esc(step.text)}</p>`);
  } else {
    parts.push(`<p>${esc(describe(step))}</p>`);
  }

  for (const incident of incidents) {
    const severity = incident.severity === 'critical' ? '' : incident.severity === 'info' ? 'info' : 'warn';
    parts.push(`
      <div class="incident ${severity}">
        <h5>${esc(incident.severity)} · ${esc(incident.kind)}</h5>
        <div class="evidence"><strong>${esc(incident.title)}</strong><br />${esc(incident.evidence)}</div>
        <div class="why">${esc(incident.why_it_matters)}</div>
      </div>
    `);
  }

  if (Array.isArray(step.hunks) && step.hunks.length > 0) {
    parts.push('<div class="hunks">');
    for (const hunk of step.hunks) {
      parts.push(renderHunk(hunk));
    }
    parts.push('</div>');
  } else if (step.kind === 'file_change') {
    parts.push('<p class="truncated">No diff stored for this change (binary, oversized, or the snapshot was pruned).</p>');
  }

  dom.detail.innerHTML = parts.join('');
}

function renderHunk(hunk) {
  let oldLine = Number(hunk.old_start) || 1;
  let newLine = Number(hunk.new_start) || 1;
  const rows = [];

  for (const line of hunk.lines || []) {
    const tag = line.tag;
    const gutter = tag === 'remove' ? oldLine : newLine;
    const sign = tag === 'add' ? '+' : tag === 'remove' ? '−' : ' ';
    rows.push(
      `<tr class="${tag === 'context' ? '' : esc(tag)}"><td class="gutter">${esc(gutter)}</td><td class="sign">${sign}</td><td>${esc(line.text)}</td></tr>`,
    );
    if (tag === 'add') newLine += 1;
    else if (tag === 'remove') oldLine += 1;
    else {
      oldLine += 1;
      newLine += 1;
    }
  }

  return `
    <div class="hunk">
      <div class="hunk-head"><span>−${esc(hunk.old_start)} +${esc(hunk.new_start)}</span><span>${pl((hunk.lines || []).length, 'line', 'lines')}</span></div>
      <table><tbody>${rows.join('')}</tbody></table>
    </div>
  `;
}

/* ── files ──────────────────────────────────────────────────────────────── */

function renderFiles() {
  const session = state.session;
  const entries = Object.entries(session.files || {}).sort((a, b) => {
    if (b[1].churn !== a[1].churn) return b[1].churn - a[1].churn;
    return a[0].localeCompare(b[0]);
  });
  const maxChurn = entries.reduce((max, [, summary]) => Math.max(max, summary.churn), 1);
  dom.files.innerHTML = '';

  dom.filesHint.textContent = entries.length
    ? `${pl(entries.length, 'file', 'files')}, most changed first`
    : 'No file changes recorded';

  for (const [path, summary] of entries) {
    const row = document.createElement('tr');
    const ops = Object.entries(summary.ops || {})
      .map(([op, count]) => `<span class="op ${esc(op)}">${esc(op)}${count > 1 ? ` ×${count}` : ''}</span>`)
      .join('');
    const prompts = (summary.prompts || []).map((id) => `<code>${esc(id)}</code>`).join(' ');
    row.innerHTML = `
      <td><code>${esc(path)}</code><span class="churn" style="width:${Math.round((summary.churn / maxChurn) * 42) + 4}px"></span></td>
      <td class="num">${esc(summary.churn)}</td>
      <td><div class="ops">${ops || '<span class="op">—</span>'}</div></td>
      <td>${prompts || '<span class="op">unattributed</span>'}</td>
    `;
    dom.files.append(row);
  }
}

boot();
