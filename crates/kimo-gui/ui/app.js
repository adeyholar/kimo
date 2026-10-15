// Kimo spectator — store, reducer, layout engine, render loop.
// Read-only: the UI only listens. Every event is folded into one state
// object; a single rAF-batched render paints the current mode.
import { connect } from './bridge.js';
import { MODES, inferMode } from './modes.js';
import { KINDS, POST, tickRunning } from './panels.js';

const DESIGN_W = 1376, DESIGN_H = 768;

export const state = {
  status: { kind: 'down', label: 'connecting' },
  session: { project: '—', agent: '—', elapsed_s: 0, t0: performance.now(), still: false },
  mode: 'edit', activity: null, explicitActivity: false, explicitMode: false,
  calls: [], callCount: 0,
  views: {},
  stats: {},
};

// ---------- reducer ----------
function merge(dst, src) {
  for (const k of Object.keys(src)) {
    const v = src[k];
    if (v && typeof v === 'object' && !Array.isArray(v) && dst[k] && typeof dst[k] === 'object' && !Array.isArray(dst[k])) merge(dst[k], v);
    else dst[k] = v;
  }
  return dst;
}
function view(mode) { return state.views[mode] || (state.views[mode] = {}); }

export function apply(ev) {
  if (!ev || typeof ev !== 'object') return;
  switch (ev.type) {
    case 'session':
      if (ev.reset) { state.calls = []; state.callCount = 0; state.views = {}; state.stats = {}; state.explicitActivity = false; state.explicitMode = false; state.activity = null; }
      for (const k of ['project', 'agent', 'read_only']) if (ev[k] !== undefined) state.session[k] = ev[k];
      if (ev.elapsed_s !== undefined) { state.session.elapsed_s = ev.elapsed_s; state.session.t0 = performance.now(); }
      if (ev.still !== undefined) state.session.still = !!ev.still;
      break;
    case 'mode':
      if (ev.mode && MODES[ev.mode]) { if (state.mode !== ev.mode) layoutDirty = true; state.mode = ev.mode; state.explicitMode = true; }
      if (ev.activity !== undefined) { state.activity = ev.activity; state.explicitActivity = true; }
      break;
    case 'view':
      merge(view(ev.mode || state.mode), ev.data || {});
      break;
    case 'stats':
      merge(state.stats, ev); delete state.stats.type;
      break;
    case 'tool_start': {
      const m = ev.mode || (state.explicitMode ? null : inferMode(ev.name));
      if (m && MODES[m] && m !== state.mode) { state.mode = m; layoutDirty = true; }
      const c = { id: ev.id, name: ev.name, args: ev.args, t: performance.now() - (ev.elapsed_ms || 0), elapsed_ms: ev.elapsed_ms, dur_ms: null, status: 'running', progress: ev.progress };
      const i = state.calls.findIndex(x => x.id === ev.id && ev.id !== undefined);
      if (i >= 0) state.calls[i] = c; else { state.calls.push(c); state.callCount++; }
      if (state.calls.length > 400) state.calls.splice(0, state.calls.length - 400);
      if (ev.count !== undefined) state.callCount = ev.count;
      break;
    }
    case 'tool_end': {
      const c = [...state.calls].reverse().find(x => x.id === ev.id) || state.calls[state.calls.length - 1];
      if (c) { c.dur_ms = ev.dur_ms ?? (performance.now() - c.t); c.status = ev.status || 'ok'; }
      break;
    }
    case 'tool_progress': {
      const c = [...state.calls].reverse().find(x => x.id === ev.id);
      if (c) merge(c, { progress: ev.progress });
      break;
    }
    case 'frame_rendered': {
      const v = view(state.mode); v.viewer = v.viewer || {};
      if (ev.png_b64) v.viewer.src = 'data:image/png;base64,' + ev.png_b64;
      else if (ev.jpg_b64) v.viewer.src = 'data:image/jpeg;base64,' + ev.jpg_b64;
      else if (ev.src) v.viewer.src = ev.src;
      for (const k of ['tc', 'res', 'tag', 'left', 'right']) if (ev[k] !== undefined) v.viewer[k] = ev[k];
      if (ev.frame !== undefined) v.viewer.frame = ev.frame;
      break;
    }
    case 'project_changed':
      state.stats.saved_at = performance.now(); state.stats.rev = ev.rev;
      break;
    default: return;
  }
  dirty = true;
}

// ---------- layout engine ----------
// A mode lists its panels as rectangles measured on the 1376x768 mockup.
// The engine turns the set of edges into CSS grid tracks: tracks inside the
// viewer's span flex (fr), everything else stays fixed px. At the design size
// every box lands on the mockup's pixel; at other sizes only the viewer grows.
let layoutDirty = true;
const main = document.getElementById('main');
const nodes = {};

function tracks(edges, flexLo, flexHi) {
  const out = [];
  for (let i = 0; i < edges.length - 1; i++) {
    const a = edges[i], b = edges[i + 1], w = b - a;
    out.push(a >= flexLo && b <= flexHi ? `minmax(0,${w}fr)` : `${w}px`);
  }
  return out.join(' ');
}

function buildLayout() {
  const M = MODES[state.mode];
  document.body.className = M.cls || '';
  const root = document.getElementById('app');
  const top = M.top ?? 46, foot = M.foot ?? 0;
  root.style.setProperty('--top', top + 'px');
  root.style.setProperty('--foot', foot + 'px');
  for (const [k, v] of Object.entries(M.vars || {})) document.documentElement.style.setProperty(k, v);
  for (const k of (window.__lastVars || [])) if (!(M.vars || {})[k]) document.documentElement.style.removeProperty(k);
  window.__lastVars = Object.keys(M.vars || {});
  const H = DESIGN_H - top - foot;
  const xs = new Set([0, DESIGN_W]), ys = new Set([0, H]);
  const rects = {};
  for (const [name, r] of Object.entries(M.rects)) {
    const rr = [r[0], r[1] - top, r[2], r[3] - top];
    rects[name] = rr; xs.add(rr[0]); xs.add(rr[2]); ys.add(rr[1]); ys.add(rr[3]);
  }
  const X = [...xs].sort((a, b) => a - b), Y = [...ys].sort((a, b) => a - b);
  const fl = rects[M.flex || 'viewer'] || [0, 0, DESIGN_W, H];
  main.style.gridTemplateColumns = tracks(X, fl[0], fl[2]);
  main.style.gridTemplateRows = tracks(Y, fl[1], fl[3]);
  main.innerHTML = '';
  for (const k of Object.keys(nodes)) delete nodes[k];
  for (const [name, spec] of Object.entries(M.panels)) {
    const r = rects[name]; if (!r) continue;
    const el = document.createElement('section');
    el.className = 'panel ' + (spec.cls || '') + ' k-' + spec.kind;
    el.dataset.panel = name;
    el.style.gridColumn = `${X.indexOf(r[0]) + 1} / ${X.indexOf(r[2]) + 1}`;
    el.style.gridRow = `${Y.indexOf(r[1]) + 1} / ${Y.indexOf(r[3]) + 1}`;
    if (spec.z) el.style.zIndex = spec.z;
    main.appendChild(el);
    nodes[name] = el;
  }
  layoutDirty = false;
}

// ---------- frame loop ----------
// Transport callbacks never touch the DOM: events are queued, and ONE
// requestAnimationFrame drains the whole queue (coalescing bursts of
// timeline/progress/frame events into a single paint). Nothing is scheduled
// while the queue is empty, and nothing at all runs while the window is
// hidden or minimized: no rAF, no timers.
const queue = [];
let rafId = 0, dirty = true, hidden = document.visibilityState === 'hidden';
export const metrics = { raf: 0, renders: 0, ticks: 0, events: 0, coalesced: 0 };
function enqueue(ev) {
  metrics.events++;
  // Hidden: fold the event into state right away (the reducer never touches
  // the DOM) so the queue cannot grow without bound; the one paint happens
  // when the window comes back.
  if (hidden) { apply(ev); return; }
  queue.push(ev); requestFrame();
}
function schedule() { dirty = true; requestFrame(); }
function requestFrame() { if (!rafId && !hidden) rafId = requestAnimationFrame(frame); }
function frame() {
  rafId = 0; metrics.raf++;
  if (queue.length > 1) metrics.coalesced += queue.length - 1;
  for (const ev of queue.splice(0)) apply(ev);
  if (dirty) { dirty = false; render(); metrics.renders++; }
  armTicker();
}
function setHidden(h) {
  hidden = h;
  if (h) { if (rafId) cancelAnimationFrame(rafId); rafId = 0; clearTimeout(tickId); tickId = 0; }
  else { dirty = true; requestFrame(); }
}
document.addEventListener('visibilitychange', () => setHidden(document.visibilityState === 'hidden'));

// ---------- ticker ----------
// The elapsed clock and a running call's duration are the only things that
// change without an event. They update text in place (no rAF, no layout
// work) from one timeout, armed only while visible and something ticks.
let tickId = 0;
function armTicker() {
  if (tickId || hidden) return;
  const running = !state.session.still && state.calls.length && state.calls[state.calls.length - 1].status === 'running';
  const clockLive = !state.session.still && state.status.kind !== 'down';
  if (!running && !clockLive) return;
  const delay = running ? 100 : 1000 - ((elapsed() * 1000) % 1000) + 5;
  tickId = setTimeout(() => {
    tickId = 0; metrics.ticks++;
    const c = document.getElementById('clock');
    if (c) c.textContent = fmtClock(elapsed());
    tickRunning(state);
    armTicker();
  }, delay);
}

function fmtClock(s) {
  s = Math.max(0, Math.floor(s));
  const h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60;
  return [h, m, x].map(n => String(n).padStart(2, '0')).join(':');
}
function elapsed() {
  const S = state.session;
  return S.still ? S.elapsed_s : S.elapsed_s + (performance.now() - S.t0) / 1000;
}

function renderTop() {
  const M = MODES[state.mode];
  document.getElementById('proj').textContent = state.session.project;
  const pill = document.getElementById('livepill');
  pill.classList.toggle('down', state.status.kind === 'down');
  document.getElementById('pill-state').textContent = state.status.kind === 'down' ? state.status.label : 'live';
  document.getElementById('pill-agent').textContent = state.session.agent;
  let act = state.activity;
  if (!state.explicitActivity || act == null) act = M.activity || state.mode;
  if (act === '#calls') act = `${state.callCount} calls`;
  document.getElementById('pill-act').textContent = act;
  const right = document.getElementById('tb-right');
  right.className = 'right ' + (M.rightCls || '');
  document.getElementById('clock').textContent = fmtClock(elapsed());
}

function renderFooter() {
  const M = MODES[state.mode];
  const f = document.getElementById('footer');
  if (!M.foot) { f.innerHTML = ''; return; }
  const st = state.stats;
  const saved = st.saved_s ?? (st.saved_at ? Math.round((performance.now() - st.saved_at) / 1000) : null);
  const parts = [];
  if (st.cpu != null) parts.push(`<span>cpu <span class="num">${String(Math.round(st.cpu)).padStart(2, ' ')}%</span></span>`);
  if (st.ffmpeg) parts.push(`<span>ffmpeg ${st.ffmpeg}</span>`);
  if (saved != null) parts.push(`<span>saved <span class="num">${saved}s</span> ago</span>`);
  f.innerHTML = parts.join('');
}

function render() {
  if (layoutDirty) buildLayout();
  renderTop();
  renderFooter();
  const M = MODES[state.mode], V = view(state.mode);
  for (const [name, spec] of Object.entries(M.panels)) {
    const el = nodes[name]; if (!el) continue;
    const fn = KINDS[spec.kind];
    if (!fn) continue;
    const w = el.clientWidth, h = el.clientHeight;
    const args = { spec, data: V[name] || {}, view: V, state, w, h, mode: state.mode };
    el.innerHTML = fn(args);
    if (POST[spec.kind]) POST[spec.kind](el, args);
  }
}

window.addEventListener('resize', () => { layoutDirty = true; schedule(); });
window.__kimo = { state, apply: enqueue, metrics, setHidden };

// Tauri: a minimized window keeps document.visibilityState on some
// platforms, so the shell's own window events gate the loop too.
async function watchTauriWindow() {
  const w = window.__TAURI__?.window?.getCurrentWindow?.();
  if (!w) return;
  const check = async () => { try { setHidden((await w.isMinimized()) || !(await w.isVisible())); } catch {} };
  w.onResized?.(check); w.onFocusChanged?.(check);
}

document.fonts.ready.then(() => {
  connect(enqueue, (kind, label) => { state.status = { kind, label }; schedule(); });
  watchTauriWindow();
  schedule();
});
