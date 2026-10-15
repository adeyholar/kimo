// Kimo spectator — store, reducer, layout engine, render loop.
// Read-only: the UI only listens. Every event is folded into one state
// object; a single rAF-batched render paints the current mode.
import { connect } from './bridge.js';
import { MODES, inferMode } from './modes.js';
import { KINDS, POST, tickRunning, pickedRows } from './panels.js';
import { route } from './router.js';

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
      if (ev.width > 0 && ev.height > 0) { state.session.width = ev.width; state.session.height = ev.height; }   // project size: the transform gizmo places x/y pixels with it
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
      if (c) { c.dur_ms = ev.dur_ms !== undefined ? ev.dur_ms : (performance.now() - c.t); /* null = the harness gave no latency: hidden */ c.status = ev.status || 'ok'; if (ev.args) c.args = ev.args; if (ev.result !== undefined) c.result = ev.result; if (ev.error) c.error = ev.error; if (ev.diff) c.diff = ev.diff; }
      break;
    }
    case 'tool_progress': {
      const c = [...state.calls].reverse().find(x => x.id === ev.id);
      if (c) merge(c, { progress: ev.progress });
      break;
    }
    case 'frame_rendered': {
      const src = ev.png_b64 ? 'data:image/png;base64,' + ev.png_b64 : ev.jpg_b64 ? 'data:image/jpeg;base64,' + ev.jpg_b64 : ev.src;
      // the engine's frame shows in this mode's viewer and in every other mode's viewer the engine
      // already owns or that has no frame yet, so a live session never hops to an empty viewer
      for (const [mode, M] of Object.entries(MODES)) for (const [name, spec] of Object.entries(M.panels)) {
        if (spec.kind !== 'viewer') continue;
        const V = view(mode), v = V[name] || (V[name] = {});
        if (mode !== state.mode && v.src && !v._engine) continue;
        if (src) { v.src = src; v._engine = true; }
        for (const k of ['tc', 'res', 'tag', 'left', 'right']) if (ev[k] !== undefined) v[k] = ev[k];
        if (ev.frame !== undefined) v.frame = ev.frame;
      }
      // a render job still running takes each new engine frame as its Renders thumbnail
      if (src) for (const r of state.views.export?.renders?.rows || []) if (r._live) r._thumb = src;
      if (src && (ev.png_b64 || ev.jpg_b64)) computeParade(src);
      break;
    }
    case 'project_changed':
      state.stats.saved_at = performance.now(); state.stats.rev = ev.rev;
      if (ev.width > 0 && ev.height > 0) { state.session.width = ev.width; state.session.height = ev.height; }
      if (typeof ev.project === 'string' && ev.project.trim()) state.session.project = ev.project.trim();
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
export const QMAX = 2048;
export const metrics = { raf: 0, renders: 0, ticks: 0, events: 0, coalesced: 0, dropped: 0, qmax: 0 };
function enqueue(ev) {
  metrics.events++;
  // Hidden: fold the event into state right away (the reducer never touches
  // the DOM) so the queue cannot grow without bound; the one paint happens
  // when the window comes back.
  if (hidden) { for (const e of route(ev, state, MODES)) apply(e); return; }
  // Bursts: progress for a call already waiting in this frame replaces the older
  // one (only the newest wipe position matters), and the queue is capped:
  // past QMAX the oldest progress goes first, then the oldest event.
  const pid = ev && (ev.phase === 'progress' ? ev.call_id : ev.type === 'tool_progress' ? ev.id : undefined);
  if (pid !== undefined) {
    for (let i = queue.length - 1; i >= 0; i--) { const q = queue[i]; if ((q.phase === 'progress' && q.call_id === pid) || (q.type === 'tool_progress' && q.id === pid)) { queue[i] = ev; metrics.coalesced++; requestFrame(); return; } }
  }
  if (queue.length >= QMAX) { const j = queue.findIndex(q => q.phase === 'progress' || q.type === 'tool_progress'); queue.splice(j >= 0 ? j : 0, 1); metrics.dropped++; }
  queue.push(ev); if (queue.length > metrics.qmax) metrics.qmax = queue.length; requestFrame();
}
// RGB parade of the engine's latest frame for the grading screen: computed once per new frame from the
// decoded image (column x value histogram per channel, log-scaled), handed to the wheels panel as an image.
// Runs only when a frame arrives; nothing ticks while the session is idle.
let paradeSeq = 0;
function computeParade(src) {
  const seq = ++paradeSeq, img = new Image();
  img.onload = () => {
    if (seq !== paradeSeq) return;   // a newer frame superseded this one
    const W = 96, H = Math.max(1, Math.round(W * img.naturalHeight / Math.max(1, img.naturalWidth)));
    const c = document.createElement('canvas'); c.width = W; c.height = H;
    const g = c.getContext('2d', { willReadFrequently: true }); g.drawImage(img, 0, 0, W, H);
    const px = g.getImageData(0, 0, W, H).data, BINS = 128, hist = [0, 1, 2].map(() => new Float32Array(W * BINS));
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) { const i = (y * W + x) * 4; for (let k = 0; k < 3; k++) hist[k][x * BINS + (px[i + k] >> 1)]++; }
    const OW = W * 3 + 8, out = document.createElement('canvas'); out.width = OW; out.height = BINS;
    const o = out.getContext('2d'), id = o.createImageData(OW, BINS), d = id.data, norm = Math.log1p(H * .35);
    const COL = [[255, 96, 72], [110, 214, 120], [92, 140, 255]];
    for (let k = 0; k < 3; k++) for (let x = 0; x < W; x++) for (let b = 0; b < BINS; b++) {
      const v = hist[k][x * BINS + b]; if (!v) continue;
      const a = Math.min(1, Math.log1p(v) / norm), j = ((BINS - 1 - b) * OW + k * (W + 4) + x) * 4;
      d[j] = COL[k][0]; d[j + 1] = COL[k][1]; d[j + 2] = COL[k][2]; d[j + 3] = Math.round(40 + 215 * a);
    }
    o.putImageData(id, 0, 0);
    for (const [mode, M] of Object.entries(MODES)) for (const [name, spec] of Object.entries(M.panels)) if (spec.kind === 'wheels') { const V = view(mode); (V[name] || (V[name] = {})).parade = { src: out.toDataURL('image/png'), live: true }; }
    schedule();
  };
  img.src = src;
}
function schedule() { dirty = true; requestFrame(); }
function requestFrame() { if (!rafId && !hidden) rafId = requestAnimationFrame(frame); }
function frame() {
  rafId = 0; metrics.raf++;
  if (queue.length > 1) metrics.coalesced += queue.length - 1;
  for (const ev of queue.splice(0)) for (const e of route(ev, state, MODES)) apply(e);
  if (dirty) { dirty = false; render(); metrics.renders++; }
  // meter ballistics: frames only while a hit/decay window is open, then nothing
  if (state.animUntil && performance.now() < state.animUntil) { dirty = true; requestFrame(); } else state.animUntil = 0;
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
  { const pj = state.session.project, has = pj != null && String(pj).trim() !== '' && pj !== '—', pe = document.getElementById('proj'); pe.textContent = has ? pj : ''; pe.style.display = has ? '' : 'none'; if (pe.previousElementSibling) pe.previousElementSibling.style.display = has ? '' : 'none'; }   // no '/ —' without a project name
  const pill = document.getElementById('livepill');
  pill.classList.toggle('down', state.status.kind === 'down');
  document.getElementById('pill-state').textContent = state.status.kind === 'down' ? state.status.label : 'live';
  const ag = state.session.agent, agEl = document.getElementById('pill-agent'), hasAg = ag != null && String(ag).trim() !== '' && ag !== '—';
  agEl.textContent = hasAg ? ag : ''; agEl.style.display = hasAg ? '' : 'none'; if (agEl.previousElementSibling) agEl.previousElementSibling.style.display = hasAg ? '' : 'none';   // no empty segment between hyphens
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
  const pickCall = state.pick != null ? state.calls.find(c => c.id === state.pick) : null;
  const kvs = Object.entries(M.panels).filter(([, sp]) => sp.kind === 'kv'), pickPanel = pickCall ? ((kvs.find(([, sp]) => sp.title === 'Inspector') || kvs[0] || [])[0]) : null;
  for (const [name, spec] of Object.entries(M.panels)) {
    const el = nodes[name]; if (!el) continue;
    const fn = KINDS[spec.kind];
    if (!fn) continue;
    const w = el.clientWidth, h = el.clientHeight;
    let data = V[name] || {};
    if (name === pickPanel && pickCall) data = { sub: `${pickCall.name}${pickCall.status === 'running' ? '' : ''}`, blocks: pickedRows(pickCall) };   // a clicked rail row, in the existing inspector
    const args = { spec, data, view: V, state, w, h, mode: state.mode };
    el.innerHTML = fn(args);
    if (POST[spec.kind]) POST[spec.kind](el, args);
  }
}

window.addEventListener('resize', () => { layoutDirty = true; schedule(); });
// click a rail row: its args / latency / result in the existing inspector; click it again or Esc to go back
main.addEventListener('click', e => { const r = e.target.closest && e.target.closest('.call[data-call]'); if (!r) return; const id = r.dataset.call, c = state.calls.find(x => String(x.id) === id); state.pick = c && state.pick !== c.id ? c.id : null; schedule(); });
document.addEventListener('keydown', e => { if (e.key === 'Escape' && state.pick != null) { state.pick = null; schedule(); } });
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
