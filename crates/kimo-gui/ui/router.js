// Tool-call router. Every engine event, in either wire format, lands here
// before the reducer:
//   unified (the engine contract, README "Unified tool events"):
//     {tool_name, call_id, phase: "start"|"progress"|"complete"|"error",
//      target: {track_id, clip_id, param}, payload: {...}, ts}
//   legacy (demo feed / older engines): {type: "tool_start"|"tool_end"|...}
// A unified event becomes the legacy rail events (so the rail always gets an
// entry) plus a family effect on the panel the tool touches: timeline clips,
// track headers, inspector rows, the viewer. Tools with no specific mapping
// still get the rail, an inspector diff of their payload and a HUD pulse, so
// no tool call is ever invisible. Pure data: nothing here touches the DOM.

import { READONLY } from './modes.js';

export const FAMILIES = [
  ['ingest', /^(import|ingest|add_media|relink|load_media|add_clip|add_track|insert)/],
  ['edit', /^(cut|split|blade|razor|trim|extend|ripple|slip|slide|move_clip|delete_clip|lift|roll)/],
  ['effects', /^(add_effect|apply_effect|set_effect|add_transition|remove_effect|add_speed|set_posterize|add_adjustment)/],
  ['transform', /^(set_property|set_clip_property|set_props?$|transform|set_transform|keyframe|set_keyframe|add_keyframe|scale|rotate|position|set_motion|add_null|set_parent|set_expression)/],
  ['color', /^(grade|color|colour|lut|apply_lut|set_lut|ocio|render_scope|analyze_exposure|wheel|curve|node_|hdr|render_ocio|render_exposure|render_gamut)/],
  ['audio', /^(set_|add_|apply_|auto_|measure_|normalize_|analyze_)?(mix|audio|gain|fader|mute|solo|pan|compress|eq|limiter|limit|loudness|duck|stem|bus|route_)/],
  ['text', /^(caption|subtitle|add_caption|add_subtitle|edit_caption|transcript_edit|transcri|add_title|title|text|set_text|add_text|set_caption|validate_captions)/],
  ['3d', /^(add_mesh|mesh|model|extrude|bevel|light|camera|particle|emit|simulate|scene|render_3d|add_light|set_light|solve_camera|track)/],
  ['export', /^(export|render|queue_render|encode|deliver)/],
];
export function family(name) {
  const n = String(name || '').toLowerCase();
  if (READONLY.test(n)) return 'inspect';
  for (const [f, re] of FAMILIES) if (re.test(n)) return f;
  return 'other';
}

const now = () => performance.now();
const FLASH_MS = 700;
export const flashing = t => t != null && now() - t < FLASH_MS;

// A value as the inspector shows it. Numbers and percent strings both parse:
// "104%" stays 104%, a scale of 1.12 reads 112%, other numbers lose float noise.
// Never returns "undefined" / "NaN" / "null".
export function fmtVal(v, param = '') {
  if (v == null) return '';
  if (typeof v === 'boolean') return v ? 'on' : 'off';
  const s = String(v).trim(), pct = /^(-?\d+(?:\.\d+)?)\s*%$/.exec(s);
  const n = pct ? +pct[1] : (typeof v === 'number' || /^-?\d+(?:\.\d+)?$/.test(s) ? +s : NaN);
  if (!Number.isFinite(n)) return Array.isArray(v) ? v.map(x => fmtVal(x)).join(', ') : (typeof v === 'object' ? '' : s);
  const round = x => String(Math.round(x * 1000) / 1000);
  if (pct) return round(n) + '%';
  if (/scale|opacity|zoom/i.test(param) && Math.abs(n) <= 20) return round(n * 100) + '%';
  return round(n);
}

function views(state) { return state.views[state.mode] || (state.views[state.mode] = {}); }
function panels(state, MODES, kinds) {
  const M = MODES[state.mode]; const V = views(state); const out = [];
  for (const [name, spec] of Object.entries(M.panels)) if (kinds.includes(spec.kind)) out.push(V[name] || (V[name] = {}));
  return out;
}
function findTrack(tl, id) {
  if (!tl || !tl.tracks) return null;
  return tl.tracks.find(t => t.id === id || t.label === id) || null;
}
function findClip(tl, target) {
  for (const t of (tl && tl.tracks) || []) {
    if (target.track_id && !(t.id === target.track_id || t.label === target.track_id)) continue;
    const i = (t.clips || []).findIndex(c => c.id === target.clip_id || c.label === target.clip_id);
    if (i >= 0) return [t, i];
  }
  return [null, -1];
}

// Inspector diff: show what a tool changed, in place if the row exists.
function inspectorDiff(state, MODES, ev, fam) {
  const P = ev.payload || {}, T = ev.target || {};
  const kv = panels(state, MODES, ['kv'])[0]; if (!kv) return false;
  kv.blocks = kv.blocks || [];
  const param = T.param || P.param;
  const label = k => String(k).replace(/_/g, ' ').replace(/^./, c => c.toUpperCase());
  const set = (k, to, from) => {
    const row = kv.blocks.find(b => b.k && b.k.toLowerCase() === String(k).toLowerCase().replace(/_/g, ' '));
    if (row) { if (from != null || row.v != null) row.from = from != null ? fmtVal(from, k) : row.v; row.v = fmtVal(to, k); row._flash = now(); return true; }
    return false;
  };
  if (param != null && (P.to != null || P.value != null)) { if (set(param, P.to ?? P.value, P.from)) return true; }
  // Unknown or unmapped: a small section for the latest call's payload (replaces the previous one).
  const i = kv.blocks.findIndex(b => b._live); if (i >= 0) kv.blocks.splice(i);
  // what changed first (param / from / to / effect / label / text), bookkeeping (timing, bounds, diffs) never
  const FIRST = ['param', 'from', 'to', 'value', 'effect', 'label', 'text', 'on', 'at', 'clip', 'track'];
  const HIDE = /^(dur_ms|duration_ms|duration|start|end|s|e|changed|mutated|result|images|ts|ts_ms|stage|elapsed_ms|progress|pct)$/;
  const rank = k => { const i = FIRST.indexOf(k); return i < 0 ? FIRST.length : i; };
  const rows = Object.entries(P).filter(([k, v]) => v != null && typeof v !== 'object' && !HIDE.test(k)).sort((a, b) => rank(a[0]) - rank(b[0])).slice(0, 4);
  const val = P.to ?? P.value, head = param != null && val != null;
  const rest = head ? rows.filter(([k]) => !['param', 'from', 'to', 'value'].includes(k)).slice(0, 3) : rows;
  if (!head && !rest.length) return false;
  kv.blocks.push({ sec: ev.tool_name, _live: true });
  // the change itself leads: "Amount 0 → 0.35"
  if (head) kv.blocks.push({ k: label(param), v: fmtVal(val, param), ...(P.from != null ? { from: fmtVal(P.from, param) } : {}), _live: true, _flash: now() });
  for (const [k, v] of rest) kv.blocks.push({ k: label(k), v: fmtVal(v, k), _live: true, _flash: now() });
  return true;
}

function hud(state, ev, fam, text) {
  state.hud = { fam, tool: ev.tool_name, text: text || ev.tool_name, phase: ev.phase, t: now() };
}

// Timeline effects of edit / ingest / effects / audio / text tools.
// The timeline a call lands on: the current mode's, else the edit mode's
// (so a real session that hops modes still builds one timeline).
function timelineView(state, MODES) {
  if (state.liveTL) return state.liveTL;
  const here = panels(state, MODES, ['timeline'])[0];
  if (here && here.tracks && here.tracks.length) return here;       // a scenario's own timeline
  // cold (engine) session: ONE timeline object, installed in every mode that draws a timeline,
  // so hopping modes as the tools change never shows a different or empty timeline
  const tl = state.liveTL = here || {};
  for (const [mode, M] of Object.entries(MODES)) for (const [name, spec] of Object.entries(M.panels)) {
    if (spec.kind !== 'timeline') continue;
    const V = state.views[mode] || (state.views[mode] = {});
    if (!V[name] || !(V[name].tracks && V[name].tracks.length)) V[name] = tl;
  }
  return tl;
}
// A real session starts empty: tracks and clips are created when a call
// first names them. New tracks append below existing ones (fixed row height
// inside a fixed panel), so nothing already on screen moves.
function ensureTrack(tl, id, hint) {
  tl.tracks = tl.tracks || [];
  let t = findTrack(tl, id);
  if (!t && id != null) {
    const audio = /audio|^a\d|music|dialog|sfx|stem/i.test(String(hint || id));
    const n = tl.tracks.filter(x => (x.kind || 'v') === (audio ? 'a' : 'v')).length + 1;
    t = { id, label: /^track\d+$/i.test(id) || !isNaN(id) ? (audio ? 'A' : 'V') + n : String(id), kind: audio ? 'a' : 'v', clips: [], _flash: now() };
    tl.tracks.push(t); tl._auto = tl._auto ?? !tl.ruler;
  }
  return t;
}
// An auto (engine-built) timeline's ruler grows to fit its clips; a scenario's ruler is left alone.
function fitRuler(tl) {
  if (!tl._auto) return;
  let e = 0; for (const t of tl.tracks || []) for (const c of t.clips || []) e = Math.max(e, +c.e || 0);
  const t1 = Math.max(10, Math.ceil(e * 1.15));
  if (!tl.ruler || tl.ruler.t1 < t1) tl.ruler = { t0: 0, t1, step: t1 <= 12 ? 2 : t1 <= 30 ? 5 : t1 <= 90 ? 10 : 30, edge: true };
}
// Clip names a person reads: a file name when the call carried a path, the text of a text clip,
// the engine's own label when it is a real name, else "V1 · Clip 02" (never a raw engine id).
const RAW_ID = /^(clip|track|fx|effect|node)_?\d+$/i;
function clipLabel(P, t, fam) {
  const A = P.args || {};
  if (fam === 'text' && P.text) return P.text;
  if (P.label && !RAW_ID.test(String(P.label))) return String(P.label);
  const path = P.path ?? P.file ?? P.src ?? A.path ?? A.file ?? A.source ?? A.src;
  if (typeof path === 'string' && path) return path.split(/[\\/]/).pop();
  return `${t.label} · Clip ${String((t.clips || []).length + 1).padStart(2, '0')}`;
}
function insertClip(tl, t, c) {
  const s = +c.s, e = +c.e;
  // overwrite edit: whatever sat under the new clip is trimmed back to it
  t.clips = (t.clips || []).filter(k => !(k.s >= s && k.e <= e) || k.id === c.id);
  for (const k of t.clips) { if (k.id === c.id) continue; if (k.s < s && k.e > s) k.e = s; else if (k.s < e && k.e > e) k.s = e; }
  const i = t.clips.findIndex(k => k.id != null && k.id === c.id);
  if (i >= 0) Object.assign(t.clips[i], c); else t.clips.push(c);
  t.clips.sort((a, b) => a.s - b.s);
}

function timelineOps(state, MODES, ev, fam) {
  const tl = timelineView(state, MODES); if (!tl) return false;
  const hit = timelineOps1(tl, ev, fam);
  if (hit) {
    fitRuler(tl);
    const P = ev.payload || {};
    // playhead follows the engine's own offsets: a cut point, else the start of the clip just touched
    const at = P.at ?? (fam === 'ingest' || fam === 'text' ? P.e : null);
    if (at != null && Number.isFinite(+at)) tl.playhead = +at;
  }
  return hit;
}
function timelineOps1(tl, ev, fam) {
  const P = ev.payload || {}, T = ev.target || {}, n = String(ev.tool_name).toLowerCase();
  const ph = P.at ?? tl.playhead;
  // add_track: an empty lane appears (engine reports it under changed.tracks_added)
  const added = (P.changed && P.changed.tracks_added) || [];
  if (/^add_track/.test(n) || (added.length && P.s == null)) {
    const ids = added.length ? added.map(x => (x && typeof x === 'object') ? (x.track_id ?? x.id) : x) : [T.track_id || P.track || 'track' + ((tl.tracks || []).length + 1)];
    for (const id of ids) ensureTrack(tl, id, P.kind || P.type);
    return true;
  }
  // any insert the engine reports (clip + track + s/e): import, add_clip, add_text, add_audio...
  if (fam !== 'edit' && P.clip != null && P.s != null && (P.track != null || T.track_id != null)) {
    const t = ensureTrack(tl, P.track ?? T.track_id, P.kind || P.source_type || n);
    insertClip(tl, t, { id: P.clip, label: clipLabel(P, t, fam), s: +P.s, e: +(P.e ?? +P.s + 4), thumb: P.thumb, ...(fam === 'text' ? { cls: 'cap' } : {}), _flash: now() });
    t._flash = now(); return true;
  }
  if (!tl.tracks) tl.tracks = [];
  if (fam === 'ingest' && P.s != null) {
    const t = ensureTrack(tl, T.track_id || P.track || 'V' + (tl.tracks.length + 1));
    insertClip(tl, t, { id: T.clip_id || P.clip, label: clipLabel(P, t, fam), s: +P.s, e: +(P.e ?? +P.s + 4), thumb: P.thumb, _flash: now() });
    t._flash = now(); return true;
  }
  let [t, i] = findClip(tl, T);
  // a call on a clip this window has not seen yet (joined mid-session): the engine sent its bounds, so draw it first
  if (!t && T.clip_id && T.track_id && (fam === 'edit' || fam === 'effects') && P.s != null && P.e != null && !/^(split|cut|blade|razor)/.test(n)) {
    t = ensureTrack(tl, T.track_id); insertClip(tl, t, { id: T.clip_id, label: clipLabel({}, t, fam), s: +P.s, e: +P.e }); i = t.clips.findIndex(c => c.id === T.clip_id);
  }
  if (fam === 'edit' && t) {
    const c = t.clips[i];
    if (/^(cut|split|blade|razor)/.test(n) && ph > c.s && ph < c.e) {
      const right = { ...c, id: P.clip != null && P.clip !== c.id ? P.clip : (c.id || c.label) + '_b', label: c.label, s: ph, _flash: now() };
      delete right.thumb;
      c.e = ph; c._flash = now();
      t.clips.splice(i + 1, 0, right);
      tl.razor = { at: ph, t: now() };
      if (P.ripple) for (const k of t.clips.slice(i + 2)) { k.s += +P.ripple; k.e += +P.ripple; }
      return true;
    }
    if (/^(trim|extend|roll|slip)/.test(n)) {
      if (P.s != null || P.in != null) c.s = +(P.s ?? P.in);
      if (P.e != null || P.out != null) c.e = +(P.e ?? P.out);
      c._flash = now(); return true;
    }
    if (/^(ripple|delete|lift)/.test(n)) {
      const gap = c.e - c.s; t.clips.splice(i, 1);
      if (!/^lift/.test(n)) for (const k of t.clips.slice(i)) { k.s -= gap; k.e -= gap; k._flash = now(); }
      return true;
    }
    if (/^(move|slide)/.test(n) && P.s != null) { const d = +P.s - c.s; c.s += d; c.e += d; c._flash = now(); return true; }
  }
  if (fam === 'effects' && t && /^(add|apply)_/.test(n)) {
    const c = t.clips[i]; const fx = P.effect || P.name || P.type || n.replace(/^(add|apply)_(effect_?)?/, '');
    c.fx = [...(c.fx || []).filter(x => x !== fx), fx]; c._flash = now(); return true;
  }
  if (fam === 'audio' && /^(mute|solo)/.test(n)) {
    const tr = findTrack(tl, T.track_id); if (!tr) return false;
    const k = n.startsWith('mute') ? 'mute' : 'solo'; tr[k] = P.on ?? P.value ?? !tr[k]; tr._flash = now(); return true;
  }
  if (fam === 'text' && P.text != null && P.s != null) {
    const tr = findTrack(tl, T.track_id || 'CAP'); if (!tr) return false;
    tr.clips = tr.clips || []; tr.clips.push({ s: +P.s, e: +(P.e ?? +P.s + 2), label: P.text, cls: 'cap', _flash: now() }); return true;
  }
  return false;
}


// Audio family on the mixing screen's own panels (m04): EQ nodes, compressor curve + gain
// reduction, Pan row, loudness readout and meter ballistics. Nothing here adds a panel.
const num = v => { const n = parseFloat(v); return Number.isFinite(n) ? n : null; };
export function fmtPan(v) { const n = num(v); if (n == null) return String(v ?? ''); const p = Math.round(Math.abs(n) <= 1 ? n * 100 : n); return p === 0 ? 'C' : (p < 0 ? 'L' : 'R') + Math.abs(p); }
function audioOps(state, MODES, ev) {
  const P = ev.payload || {}, A = P.args || {}, n = String(ev.tool_name).toLowerCase(), t = now();
  const V = views(state), M = MODES[state.mode]; let hit = false;
  const get = k => P[k] ?? A[k];
  const kvs = Object.entries(M.panels).filter(([, s]) => s.kind === 'kv').map(([k]) => V[k]).filter(Boolean);
  const blocks = kvs.flatMap(k => k.blocks || []);
  const eq = blocks.find(b => b.widget === 'eq'), comp = blocks.find(b => b.widget === 'comp');
  if (eq && (/eq/.test(n) || /eq/.test(String(get('effect') ?? '')))) {
    const f = num(get('freq') ?? get('f') ?? get('frequency')), g = num(get('gain') ?? get('g') ?? get('to')), q = num(get('q'));
    const bands = (eq.bands || []).filter(B => B.g != null && !/pass/.test(B.type || ''));
    let B = get('band') != null ? bands[+get('band')] : null;
    if (!B && f != null) B = bands.reduce((best, x) => (!best || Math.abs(Math.log(x.f / f)) < Math.abs(Math.log(best.f / f)) ? x : best), null);
    B = B || bands.find(x => x.sel) || bands[0];
    if (B) { if (f != null) B.f = f; if (g != null) B.g = g; if (q != null) B.q = q; bands.forEach(x => { x.sel = x === B; }); B._flash = t; eq._flash = t; hit = true; }
  }
  if (comp && /compress|comp(?!ute)|limit/.test(n)) {
    const th = num(get('threshold')), ra = num(get('ratio')), inp = num(get('input_db') ?? get('level'));
    if (th != null && th !== comp.threshold) { comp.threshold = th; comp._flashT = t; }
    if (ra != null && ra !== comp.ratio) { comp.ratio = ra; comp._flashR = t; }
    comp.inDb = inp ?? Math.min(0, (comp.threshold ?? -18) + 8);   // where the signal sits on the curve while the call runs
    comp._grT = t; hit = true;
  }
  if (/pan/.test(n)) {
    const v = get('pan') ?? get('value') ?? get('to');
    const row = blocks.find(b => b.k && /^pan$/i.test(b.k));
    if (row && v != null) { const nv = fmtPan(v); if (nv !== row.v) { row.from = row.v; row.v = nv; } row._flash = t; hit = true; }
  }
  // loudness readout + meters: every audio call moves the meters (a hit then decay); loudness tools set the number
  const L = Object.entries(M.panels).filter(([, s]) => s.kind === 'loudness').map(([k]) => V[k] || (V[k] = {}))[0];
  if (L) {
    const lufs = num(P.lufs ?? P.to ?? get('target_lufs') ?? get('lufs'));
    if (/loud|normaliz|lufs/.test(n) && lufs != null) { L.lufs = lufs.toFixed(1); L._flash = t; }
    const gain = num(get('gain') ?? get('gain_db')) ?? 0;
    for (const m of (L.meters || [])) {
      const base = m.v ?? .5, lvl = Math.max(0, Math.min(1, base + .12 + gain * .03));
      m.hit = { t, lvl, base };
      if (m.peak == null || lvl > m.peak) m.peakHold = { t, lvl };
    }
    state.animUntil = Math.max(state.animUntil || 0, t + METER_MS);
    hit = true;
  }
  return hit;
}
export const METER_MS = 1600;   // hit + VU decay + peak-hold fall: frames run only inside this window


// Export family on the export screen's own panels (m14): the viewer's progress bar (frame a / b,
// fps, ETA), the Export settings rows the call actually carried, and a job row in Renders that
// counts up while rendering and stays in the list (history) when it lands.
const mmss = ms => { const s = Math.max(0, Math.round(ms / 1000)); return String(Math.floor(s / 60)).padStart(2, '0') + ':' + String(s % 60).padStart(2, '0'); };
const shortPath = (p, max = 24) => { let s = String(p).replace(/^\/(home|Users)\/[^/]+/, '~'); const seg = s.split('/'); return s.length > max && seg.length > 3 ? '…/' + seg.slice(-(max > 24 ? 3 : 2)).join('/') : s; };
const fmtBytes = b => b >= 1e9 ? (b / 1e9).toFixed(2) + ' GB' : b >= 1e6 ? (b / 1e6).toFixed(1) + ' MB' : b >= 1e3 ? Math.round(b / 1e3) + ' KB' : b + ' B';
function exportOps(state, MODES, ev) {
  const M = MODES.export; if (!M) return false;
  const V = state.views.export || (state.views.export = {});
  const P = ev.payload || {}, A = P.args || {}, id = ev.call_id ?? ev.tool_name;
  const name = Object.keys(M.panels), kvName = name.find(k => M.panels[k].kind === 'kv'), rName = name.find(k => M.panels[k].kind === 'renders'), vName = name.find(k => M.panels[k].kind === 'viewer');
  const exp = V[kvName] || (V[kvName] = {}), ren = V[rName] || (V[rName] = {}), vw = V[vName] || (V[vName] = {});
  ren.head = ren.head || ['name', 'preset', 'length', 'status', 'started'];
  ren.rows = ren.rows || [];
  let row = ren.rows.find(r => r._call === id);
  if (ev.phase === 'start') {
    const path = A.path ?? A.output ?? A.file ?? '';
    const base = String(path).split(/[\\/]/).pop() || ev.tool_name;
    const ext = (base.match(/\.([a-z0-9]+)$/i) || [])[1];
    const res = A.resolution ?? (A.width && A.height ? `${A.width} x ${A.height}` : null);
    const codec = A.codec ?? A.video_codec ?? null;   // only what the call carried: nothing is guessed
    const set = [['Format', A.container ?? A.format ?? (ext ? ext.toUpperCase() : null)], ['Video', codec], ['Resolution', res], ['Frame rate', A.fps], ['Quality', A.crf != null ? 'CRF ' + A.crf : A.quality], ['Audio', A.audio === false ? 'off' : A.audio_codec], ['Output', path ? shortPath(path) : null]];
    exp.blocks = exp.blocks || [];
    for (const [k, v] of set) { if (v == null || v === '') continue; const b = exp.blocks.find(x => x.k === k); if (b) { if (b.v !== String(v)) { b.from = b.v; b.v = String(v); } b._flash = now(); } else exp.blocks.push({ k, v: String(v), _flash: now() }); }
    const preset = [res && String(res).replace(/\s*x\s*/, 'x'), codec].filter(Boolean).join(' ') || A.preset || '';
    if (!row) { row = [base, preset, '', '● rendering 0%', 'now']; row._call = id; row._t0 = now(); row._job = true; ren.rows.unshift(row); }
    row._live = true; if (path) row._path = shortPath(path, 48); if (vw.src) row._thumb = vw.src;
    vw.progress = { pct: 0, left: 'starting', right: '' };
    return true;
  }
  if (ev.phase === 'progress') {
    const pct = P.pct ?? (P.current_frame != null && P.total_frames ? 100 * P.current_frame / P.total_frames : null);
    if (pct == null) return false;
    const left = P.current_frame != null && P.total_frames ? `frame ${P.current_frame} / ${P.total_frames}` : `${Math.round(pct)}%`;
    const right = [P.fps != null ? `${Math.round(P.fps)} fps` : '', P.elapsed_ms != null ? `${mmss(P.elapsed_ms)} elapsed` : '', P.eta_ms != null ? `eta ${mmss(P.eta_ms)}` : ''].filter(Boolean).join('\u2003');
    vw.progress = { pct: Math.round(pct), left, right };
    if (row) row[3] = `● rendering ${Math.round(pct)}%`;
    return true;
  }
  const ok = ev.phase === 'complete', dur = P.dur_ms ?? ev.duration_ms;
  if (row) {
    // history row: the last frame the engine rendered, the output path, length and final size, each
    // only when the engine actually sent it (no placeholder)
    const R = P.result && typeof P.result === 'object' ? P.result : {};
    const bytes = [R.bytes, R.size_bytes, R.file_size, P.bytes, P.size_bytes].find(x => typeof x === 'number' && x > 0);
    row[3] = ok ? '✓ done' + (bytes ? ' · ' + fmtBytes(bytes) : '') : '✕ failed';
    if (ok && typeof R.duration === 'number') row[2] = mmss(R.duration * 1000);
    if (ok && R.path) row._path = shortPath(R.path, 48);
    if (vw.src) row._thumb = vw.src;
    if (dur != null) row[4] = mmss(dur); row._flash = now(); row._live = false;
  }
  if (vw.progress) vw.progress = ok ? { ...vw.progress, pct: 100, right: (dur != null ? `done in ${mmss(dur)}` : 'done') } : { ...vw.progress, right: 'failed' };
  return true;
}

// Unified event -> legacy rail events + family effects. Returns the legacy
// events the reducer should apply (the rail), having already mutated views.
export function route(ev, state, MODES) {
  if (!ev || ev.type || !ev.tool_name) return [ev];        // legacy or non-tool event: unchanged
  const fam = family(ev.tool_name), id = ev.call_id ?? ev.tool_name + '@' + (ev.ts ?? '');
  const out = [];
  if (fam === 'inspect') return inspectOps(state, MODES, ev, id);
  const T = ev.target || {}, P = ev.payload || {};
  const param = T.param || P.param, val = P.to ?? P.value;
  const args = { ...(T.clip_id ? { clip: T.clip_id } : {}), ...(param && val != null ? { [param]: fmtVal(val, param) } : {}) };
  if (ev.phase === 'start') {
    out.push({ type: 'tool_start', id, name: ev.tool_name, args: P.args ? P.args : (Object.keys(args).length ? args : undefined) });
    if (fam === 'export') exportOps(state, MODES, ev);
    hud(state, ev, fam);
  } else if (ev.phase === 'progress') {
    const pct = P.pct ?? (P.done != null && P.total ? 100 * P.done / P.total : null);
    out.push({ type: 'tool_progress', id, progress: P.done != null ? { done: P.done, total: P.total } : { pct } });
    if (fam === 'audio' && /compress|comp(?!ute)|limit/.test(String(ev.tool_name).toLowerCase())) audioOps(state, MODES, ev);
    if (fam === 'export') exportOps(state, MODES, ev);
    if (fam === 'color' && pct != null) { const v = views(state); v.viewer = v.viewer || {}; v.viewer.wipe = pct / 100; }
    hud(state, ev, fam, `${ev.tool_name} ${pct != null ? Math.round(pct) + '%' : ''}`.trim());
  } else if (ev.phase === 'complete' || ev.phase === 'error') {
    const err = ev.phase === 'error';
    const dur = P.dur_ms ?? ev.duration_ms;
    // the landed value replaces the rail's start-time args: "scale: 104% → 112%"
    const landed = !err && param && val != null ? { ...(T.clip_id ? { clip: T.clip_id } : {}), [param]: (P.from != null ? fmtVal(P.from, param) + ' → ' : '') + fmtVal(val, param) } : undefined;
    out.push({ type: 'tool_end', id, name: ev.tool_name, dur_ms: dur, status: err ? 'error' : 'ok', error: P.error, ...(landed ? { args: landed } : {}) });
    if (!err) {
      const v = views(state); if (v.viewer && v.viewer.wipe != null) v.viewer.wipe = 1;
      if (/false_?colou?r/.test(String(ev.tool_name).toLowerCase())) { v.viewer = v.viewer || {}; v.viewer.falsecolor = { split: P.split ?? .5 }; }
      const hit = timelineOps(state, MODES, ev, fam) | (fam === 'audio' ? audioOps(state, MODES, ev) : false) | (fam === 'export' ? exportOps(state, MODES, ev) : false);
      const diff = inspectorDiff(state, MODES, ev, fam);
      if (!hit && !diff) hud(state, ev, fam, ev.tool_name + ' ✓'); else hud(state, ev, fam, ev.tool_name + ' ✓');
    } else { if (fam === 'export') exportOps(state, MODES, ev); hud(state, ev, fam, ev.tool_name + ' failed'); }
  }
  return out;
}


// Read-only family (get_/list_/inspect_/probe_/sample_/query…): the call changes nothing, so nothing
// on the canvas or timeline is mutated and the workspace does not move. What the spectator sees: the
// rail entry, the HUD pill in its quiet "read" state with what came back ("get_transform · x 40, y 0"),
// and a cream peek ring on the element the call read (its clip, track label or inspector row).
const summary = r => {
  if (r == null || r === '') return '';
  if (Array.isArray(r)) return r.length + (r.length === 1 ? ' item' : ' items');
  if (typeof r !== 'object') { const s = String(r); return s.length > 36 ? s.slice(0, 35) + '…' : s; }
  const parts = [];
  for (const [k, v] of Object.entries(r)) {
    if (v == null || /^(ok|path|kimo_build|build|ts|ts_ms|dur_ms)$/.test(k)) continue;
    if (Array.isArray(v)) parts.push(`${k.replace(/_/g, ' ')} ×${v.length}`);
    else if (typeof v !== 'object') { const f = fmtVal(v, k); if (f !== '' && f.length <= 18) parts.push(`${k.replace(/_/g, ' ')} ${f}`); }
    if (parts.length === 2) break;
  }
  return parts.join(', ');
};
function inspectOps(state, MODES, ev, id) {
  const P = ev.payload || {}, T = ev.target || {}, A = P.args || {};
  const out = [];
  if (ev.phase === 'start') {
    out.push({ type: 'tool_start', id, name: ev.tool_name, args: Object.keys(A).length ? A : undefined, read: true });
    hud(state, ev, 'inspect', ev.tool_name);
    peek(state, MODES, { clip_id: T.clip_id ?? A.clip_id, track_id: T.track_id ?? A.track_id, param: T.param ?? A.param ?? A.property });
  } else if (ev.phase === 'progress') {
    const pct = P.pct;
    out.push({ type: 'tool_progress', id, progress: { pct } });
  } else {
    const err = ev.phase === 'error', dur = P.dur_ms ?? ev.duration_ms, sm = err ? '' : summary(P.result);
    out.push({ type: 'tool_end', id, name: ev.tool_name, dur_ms: dur, status: err ? 'error' : 'ok', error: P.error, read: true });
    hud(state, ev, 'inspect', ev.tool_name + (err ? ' failed' : sm ? ' · ' + sm : ' ✓'));
  }
  return out;
}
function peek(state, MODES, t) {
  const tl = state.liveTL || panels(state, MODES, ['timeline'])[0];
  if (t.clip_id != null) { const [tr, i] = findClip(tl, { clip_id: t.clip_id, track_id: t.track_id }); if (tr) tr.clips[i]._peek = now(); }
  else if (t.track_id != null) { const tr = findTrack(tl, t.track_id); if (tr) tr._peek = now(); }
  if (t.param != null) {
    const kv = panels(state, MODES, ['kv'])[0], k = String(t.param).toLowerCase().replace(/_/g, ' ');
    const row = kv && (kv.blocks || []).find(b => b.k && b.k.toLowerCase() === k); if (row) row._peek = now();
  }
}
