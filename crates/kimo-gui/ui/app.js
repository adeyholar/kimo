// Kimo spectator — store, reducer, layout engine, render loop.
// Read-only: the UI only listens. Every event is folded into one state
// object; a single rAF-batched render paints the current mode.
import { connect } from './bridge.js';
import { MODES, inferMode } from './modes.js';
import { KINDS, PRE, POST, tickRunning, pickedRows, TL_PARTS } from './panels.js';
import { route, freezeNow } from './router.js';

const DESIGN_W = 1376, DESIGN_H = 768;

export const state = {
  status: { kind: 'down', label: 'connecting' },
  session: { project: '—', agent: '—', elapsed_s: 0, t0: performance.now(), still: false },
  mode: 'edit', activity: null, explicitActivity: false, explicitMode: false,
  calls: [], callCount: 0,
  views: {},
  stats: {},
  preview: { shown: -1, want: 0, revs: false },   // the engine frame's rev on the viewer, the project's latest rev (see previewMark)
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

/* "out of date · rev N" on every viewer the engine's frames own, from the moment a mutating call moves the project past
   the frame on screen until the engine's frame for that rev (or a newer one) arrives. Off until the engine has sent a
   frame with a rev: an engine without auto-preview never gets a mark it could not clear. */
function previewMark() {
  const P = state.preview, mark = P.revs && P.want > P.shown ? P.want : null;
  for (const [mode, M] of Object.entries(MODES)) for (const [name, spec] of Object.entries(M.panels)) {
    if (spec.kind !== 'viewer') continue;
    const v = (state.views[mode] || {})[name]; if (!v || !v._engine) continue;
    if (mark != null) v.stale = mark; else delete v.stale;
  }
}

export function apply(ev) {
  if (!ev || typeof ev !== 'object') return;
  switch (ev.type) {
    case 'session':
      if (ev.reset) { state.calls = []; state.callCount = 0; state.views = {}; state.stats = {}; state.preview = { shown: -1, want: 0, revs: false }; state.explicitActivity = false; state.explicitMode = false; state.activity = null; }
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
      // the engine runs one call at a time: a new start after a feed drop means a call left 'running' across the gap
      // finished while we could not hear it; its outcome is unknown, so it says so instead of ticking forever
      if (state._gapAt != null) { for (const x of state.calls) if (x.status === 'running' && x.t < state._gapAt) { x.status = 'lost'; x.dur_ms = null; } state._gapAt = null; }
      const c = { id: ev.id, name: ev.name, args: ev.args, t: performance.now() - (ev.elapsed_ms || 0), elapsed_ms: ev.elapsed_ms, dur_ms: null, status: 'running', progress: ev.progress };
      const i = state.calls.findIndex(x => x.id === ev.id && ev.id !== undefined);
      if (i >= 0) state.calls[i] = c; else { state.calls.push(c); state.callCount++; }
      if (state.calls.length > 400) state.calls.splice(0, state.calls.length - 400);
      if (ev.count !== undefined) state.callCount = ev.count;
      break;
    }
    case 'tool_end': {
      let c = ev.id !== undefined ? [...state.calls].reverse().find(x => x.id === ev.id) : state.calls[state.calls.length - 1];
      if (!c && ev.id !== undefined) { c = { id: ev.id, name: ev.name, args: ev.args, t: performance.now(), dur_ms: null, status: 'ok' }; state.calls.push(c); state.callCount++; if (state.calls.length > 400) state.calls.splice(0, state.calls.length - 400); }   // its start was lost in a drop: the call still shows, settled
      if (c) { c.dur_ms = ev.dur_ms !== undefined ? ev.dur_ms : (performance.now() - c.t); /* null = the harness gave no latency: hidden */ c.status = ev.status || 'ok'; if (ev.args) c.args = ev.args; if (ev.result !== undefined) c.result = ev.result; if (ev.error) c.error = ev.error; if (ev.diff) c.diff = ev.diff; }
      break;
    }
    case 'tool_progress': {
      const c = [...state.calls].reverse().find(x => x.id === ev.id);
      if (c) merge(c, { progress: ev.progress });
      break;
    }
    case 'frame_rendered': {
      /* Preview safety: an engine frame carries the project rev it shows (k5 auto-preview). A frame older than the one on
         the viewer is dropped (a slow render never overwrites a newer one); a frame with no rev (an older engine) shows
         as before. The GUI never asks for a frame: it only listens. */
      const P = state.preview, fr = Number.isFinite(ev.rev) ? ev.rev : null;
      if (fr != null) { if (fr < P.shown) { metrics.staleFrames++; return; } P.shown = fr; P.revs = true; }
      else if (P.revs) P.shown = Math.max(P.shown, P.want);
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
      previewMark();
      break;
    }
    case 'project_changed':
      state.stats.saved_at = performance.now(); state.stats.rev = ev.rev;
      if (Number.isFinite(ev.rev)) { state.preview.want = Math.max(state.preview.want, ev.rev); previewMark(); }   // only a mutating call moves the rev
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
  // spring-eased splits: a panel that exists on both screens of a mode hop starts at its old box and springs
  // to its new one (transform only, so nothing else reflows); a resize or first build snaps
  const hop = builtMode != null && builtMode !== state.mode && !hidden, old = {};
  if (hop) for (const [k, el] of Object.entries(nodes)) old[k] = [el.offsetLeft, el.offsetTop, el.offsetWidth, el.offsetHeight];
  springs.length = 0;
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
  if (hop) for (const [name, el] of Object.entries(nodes)) {
    const f = old[name]; if (!f) continue;
    const t = [el.offsetLeft, el.offsetTop, el.offsetWidth, el.offsetHeight];
    if (!t[2] || !t[3] || f.every((v, i) => Math.abs(v - t[i]) < .5)) continue;
    springs.push({ el, d: [f[0] - t[0], f[1] - t[1], f[2] / t[2], f[3] / t[3]], D: Math.max(Math.abs(f[0] - t[0]), Math.abs(f[1] - t[1]), Math.abs(f[2] - t[2]), Math.abs(f[3] - t[3])) });
    el.dataset.anim = '1'; el.style.transformOrigin = '0 0';
  }
  if (springs.length) { springT0 = performance.now(); stepSprings(0); }
  builtMode = state.mode;
  layoutDirty = false;
}
// One spring per hop (all moving panels share it), critically damped (ratio 1.0, w = 60 rad/s, i.e. stiffness
// 3600): x(t) = (1 + wt) e^-wt runs 1 -> 0 with no overshoot by construction. A panel lands when its residual is
// under 0.5 px (~125 ms for a 100 px move, ~170 ms for 1000 px) and never later than 185 ms. Evaluated in closed form
// from the hop's start, so a slow frame cannot make it ring. Driven by the render loop's own rAF only while a
// spring is moving; hidden -> snapped.
const springs = []; let builtMode = null, springT0 = 0;
const pans = {}, panDirty = new Set();   /* timeline pan / zoom per mode:panel (see onWheel) */
const SW = 60, SMAX = .185;
function stepSprings(now) {
  if (!springs.length) return false;
  const t = now ? Math.max(0, (now - springT0) / 1000) : 0, x = (1 + SW * t) * Math.exp(-SW * t);
  let live = false;
  for (const s of springs) {
    if (!s.el) continue;
    if (hidden || t >= SMAX || x * s.D < .5) { s.el.style.transform = ''; delete s.el.dataset.anim; s.el = null; continue; }
    live = true; const [dx, dy, sx, sy] = s.d;
    s.el.style.transform = `translate(${dx * x}px, ${dy * x}px) scale(${1 + (sx - 1) * x}, ${1 + (sy - 1) * x})`;
  }
  if (!live) springs.length = 0;
  return live;
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
export const metrics = { raf: 0, renders: 0, scoped: 0, panelWrites: 0, textPatches: 0, trackWrites: 0, ticks: 0, events: 0, coalesced: 0, dropped: 0, qmax: 0, staleFrames: 0 };
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
// A frame that only carries split_clip / move_clip events repaints the timeline and the rail (the call's row)
// and nothing else: no layout pass, no top bar, no Inspector, no viewer. Those catch up on the next frame that
// carries no clip edit (one rAF, queued only while something is owed), so a stream of edits never repaints them.
const CLIP_EDIT = /^(split_clip|move_clip)$/;
const deferred = { on: false };
function isClipEdit(ev) { const n = ev && (ev.tool_name ?? (ev.type === 'tool_start' || ev.type === 'tool_end' || ev.type === 'tool_progress' ? ev.name : null)); return n != null && CLIP_EDIT.test(n); }
function renderScoped() { for (const [name, spec] of Object.entries(MODES[state.mode].panels)) if (spec.kind === 'timeline' || spec.kind === 'agent') renderPanel(name); }
function requestFrame() { if (!rafId && !hidden) rafId = requestAnimationFrame(frame); }
function frame() {
  rafId = 0; metrics.raf++; freezeNow(performance.now());
  try { frameBody(); } finally { freezeNow(0); }
}
function frameBody() {
  if (queue.length > 1) metrics.coalesced += queue.length - 1;
  const evs = queue.splice(0), editOnly = evs.length > 0 && !dirty && evs.every(isClipEdit);
  for (const ev of evs) for (const e of route(ev, state, MODES)) apply(e);
  if (editOnly && !layoutDirty && builtMode === state.mode) { dirty = false; renderScoped(); metrics.renders++; metrics.scoped++; deferred.on = true; requestFrame(); }
  else if (dirty || deferred.on) { dirty = false; deferred.on = false; panDirty.clear(); render(); metrics.renders++; }
  else if (panDirty.size) { for (const n of panDirty) renderPanel(n); panDirty.clear(); metrics.renders++; }
  // meter ballistics: frames only while a hit/decay window is open, then nothing
  if (state.animUntil && performance.now() < state.animUntil) { dirty = true; requestFrame(); } else state.animUntil = 0;
  if (stepSprings(performance.now())) requestFrame();   // a split still springing: one more frame
  armTicker();
}
function setHidden(h) {
  hidden = h;
  if (h) { if (rafId) cancelAnimationFrame(rafId); rafId = 0; clearTimeout(tickId); tickId = 0; stepSprings(0); }
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
  { const pj = state.session.project, has = pj != null && String(pj).trim() !== '' && pj !== '—', pe = document.getElementById('proj'); setText(pe, has ? String(pj) : ''); setDisp(pe, has ? '' : 'none'); setDisp(pe.previousElementSibling, has ? '' : 'none'); }   // no '/ —' without a project name
  const pill = document.getElementById('livepill');
  pill.classList.toggle('down', state.status.kind === 'down');
  setText(document.getElementById('pill-state'), state.status.kind === 'down' ? String(state.status.label) : 'live');
  const ag = state.session.agent, agEl = document.getElementById('pill-agent'), hasAg = ag != null && String(ag).trim() !== '' && ag !== '—';
  setText(agEl, hasAg ? String(ag) : ''); setDisp(agEl, hasAg ? '' : 'none'); setDisp(agEl.previousElementSibling, hasAg ? '' : 'none');   // no empty segment between hyphens
  let act = state.activity;
  if (!state.explicitActivity || act == null) act = M.activity || state.mode;
  if (act === '#calls') act = `${state.callCount} calls`;
  setText(document.getElementById('pill-act'), String(act));
  const right = document.getElementById('tb-right');
  const rc = 'right ' + (M.rightCls || ''); if (right.className !== rc) right.className = rc;
  setText(document.getElementById('clock'), fmtClock(elapsed()));
}

function renderFooter() {
  const M = MODES[state.mode];
  const f = document.getElementById('footer');
  if (!M.foot) { paint(f, ''); return; }
  const st = state.stats;
  const saved = st.saved_s ?? (st.saved_at ? Math.round((performance.now() - st.saved_at) / 1000) : null);
  const parts = [];
  if (st.cpu != null) parts.push(`<span>cpu <span class="num">${String(Math.round(st.cpu)).padStart(2, ' ')}%</span></span>`);
  if (st.ffmpeg) parts.push(`<span>ffmpeg ${st.ffmpeg}</span>`);
  if (saved != null) parts.push(`<span>saved <span class="num">${saved}s</span> ago</span>`);
  paint(f, parts.join(''));
}

// A picked call can carry more rows than a short Inspector holds: the tail (Result, then the last Args) gives way,
// and one dim '+N more' row says so, instead of rows running past the panel's bottom edge (only on a pick render)
function fitPicked(el) {
  const kv = el.querySelector('.kv'); if (!kv) return;
  const bottom = el.getBoundingClientRect().bottom - 6, over = () => kv.lastElementChild && kv.lastElementChild.getBoundingClientRect().bottom > bottom;
  if (!over()) return;
  let n = 0;
  while (kv.children.length > 3 && over()) { const last = kv.lastElementChild; if (last.classList.contains('row')) n++; last.remove(); }
  while (kv.lastElementChild && kv.lastElementChild.classList.contains('sec')) kv.lastElementChild.remove();   // no empty section title at the end
  const more = document.createElement('div'); more.className = 'row dim'; more.innerHTML = `<span class="k"></span><span class="v">+${n} more</span>`; kv.appendChild(more);
  while (kv.children.length > 3 && over()) { more.previousElementSibling.remove(); more.lastElementChild.textContent = `+${++n} more`; }
}
/* Write only what changed. A panel whose markup is the same as last frame keeps its DOM (and whatever its POST
   painted), so a frame that edits the timeline repaints the timeline, the rail row and the call's own text, not every
   panel. Panel boxes are read once per layout (grid tracks are fixed for a layout), so a render never forces one. */
function setText(el, v) { if (el && el.textContent !== v) el.textContent = v; }
function setDisp(el, v) { if (el && el.style.display !== v) el.style.display = v; }
function paint(el, html) {
  if (el._h === html) return false;
  const P = TL_PARTS && TL_PARTS.html === html ? TL_PARTS : null, Q = el._p;   /* a timeline: rewrite only the tracks that changed */
  el._p = P;
  if (P && Q && P.pre === Q.pre && P.tracks.length && P.tracks.length === Q.tracks.length) {
    const tl = el.querySelector(':scope > .tl'), trs = tl ? tl.querySelectorAll(':scope > .trk') : [];
    if (trs.length === P.tracks.length) {
      for (let i = 0; i < trs.length; i++) if (P.tracks[i] !== Q.tracks[i]) { trs[i].outerHTML = P.tracks[i]; metrics.trackWrites++; }
      if (P.post !== Q.post) { const last = tl.querySelectorAll(':scope > .trk'), end = last[last.length - 1]; while (end.nextSibling) end.nextSibling.remove(); if (P.post) end.insertAdjacentHTML('afterend', P.post); }
      el._h = html; return true;
    }
  }
  if (el._h != null && html.length < 4096 && !html.includes('flash') && retext(el, el._h, html)) { el._h = html; metrics.textPatches++; return false; }
  el.innerHTML = html; el._h = html; metrics.panelWrites++; return true;
}
/* Same tags and attributes, only the words differ (a viewer HUD caption naming the new call): the text nodes are
   updated in place and every element stays. Markup carrying a flash is always rebuilt, so its flash replays. */
const TAG = /<[^>]*>/g, tpl = document.createElement('template');
function texts(root) { const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT), out = []; while (w.nextNode()) out.push(w.currentNode); return out; }
function retext(el, prev, html) {
  const a = prev.match(TAG) || [], b = html.match(TAG) || [];
  if (a.length !== b.length || a.some((t, i) => t !== b[i])) return false;
  tpl.innerHTML = html; const now = texts(tpl.content), old = texts(el);
  if (now.length !== old.length) return false;
  for (let i = 0; i < now.length; i++) if (old[i].nodeValue !== now[i].nodeValue) old[i].nodeValue = now[i].nodeValue;
  return true;
}
function box(el) { return el._sz || (el._sz = [el.clientWidth, el.clientHeight]); }
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
    const [w, h] = box(el);
    let data = V[name] || {};
    if (name === pickPanel && pickCall) data = { sub: `${pickCall.name}${pickCall.status === 'running' ? '' : ''}`, blocks: pickedRows(pickCall) };   // a clicked rail row, in the existing inspector
    const args = { spec, data, view: V, state, w, h, mode: state.mode, pan: pans[state.mode + ':' + name] };
    if (PRE[spec.kind]) PRE[spec.kind](el, args);   /* measures on the clean layout, before this panel is written */
    if (!paint(el, fn(args))) continue;
    if (name === pickPanel && pickCall) fitPicked(el);
    if (POST[spec.kind]) POST[spec.kind](el, args);
  }
}

window.addEventListener('resize', () => { layoutDirty = true; schedule(); });
// Timeline pan / zoom, the minimal scroll the mockups allow (no scrollbar, no control drawn): wheel / trackpad
// sideways (or shift+wheel) pans a zoomed-in timeline, ctrl+wheel or a pinch zooms around the pointer, plain wheel
// steps the track rows so a track below the panel (V6) can be brought up. Input only queues a frame, and that
// frame redraws just the timeline panel; nothing runs between inputs.
function onWheel(e) {
  const el = e.target.closest && e.target.closest('[data-panel]'); if (!el) return;
  const name = el.dataset.panel, spec = (MODES[state.mode] || {}).panels[name]; if (!spec || spec.kind !== 'timeline') return;
  const data = view(state.mode)[name]; if (!data || !(data.tracks || []).length) return;
  const key = state.mode + ':' + name, P = pans[key] || (pans[key] = { z: 1, t: null, k: 0, acc: 0 });
  const o = spec.opts || {}, x0 = o.x0 ?? 48, wpx = Math.max(1, el.clientWidth - (o.xr ?? 12) - x0);
  const r = data.ruler || { t0: 0, t1: 20 }, full = r.t1 - r.t0, span = full / P.z, t0 = P.t ?? r.t0;
  const k = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? wpx : 1;
  let dx = e.deltaX * k, dy = e.deltaY * k;
  if (e.shiftKey && !dx) { dx = dy; dy = 0; }
  if (e.ctrlKey || e.metaKey) {
    const zmax = Math.max(1, full / 4), z = Math.min(zmax, Math.max(1, P.z * Math.exp(-dy * .004)));
    if (z === P.z) return;
    const fx = Math.min(1, Math.max(0, (e.clientX - el.getBoundingClientRect().left - x0) / wpx)), at = t0 + fx * span;
    P.z = z; P.t = z > 1 ? at - fx * full / z : null;
  } else if (Math.abs(dx) >= Math.abs(dy)) {
    if (P.z <= 1 || !dx) return;
    P.t = t0 + dx * span / wpx;
  } else {
    if (P.kmax === 0 && !P.k) return;
    P.acc += dy; const rows = Math.trunc(P.acc / 40); if (!rows) { e.preventDefault(); return; }
    P.acc -= rows * 40; P.k = (P.k || 0) + rows;
  }
  e.preventDefault(); panDirty.add(name); requestFrame();
}
main.addEventListener('wheel', onWheel, { passive: false });
function renderPanel(name) {
  const spec = (MODES[state.mode] || {}).panels[name], el = nodes[name]; if (!spec || !el) return;
  const [w, h] = box(el), V = view(state.mode), args = { spec, data: V[name] || {}, view: V, state, w, h, mode: state.mode, pan: pans[state.mode + ':' + name] };
  if (PRE[spec.kind]) PRE[spec.kind](el, args);
  if (!paint(el, KINDS[spec.kind](args))) return;
  if (POST[spec.kind]) POST[spec.kind](el, args);
}
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
  connect(enqueue, (kind, label) => { state.status = { kind, label }; if (label === 'reconnecting' && state._gapAt == null) state._gapAt = performance.now(); schedule(); });
  watchTauriWindow();
  schedule();
});
