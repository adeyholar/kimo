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

import { READONLY, inferMode } from './modes.js';

export const FAMILIES = [
  ['ingest', /^(import|ingest|add_media|relink|load_media|add_clip|add_track|insert)/],
  ['edit', /^(cut|split|blade|razor|trim|extend|ripple|slip|slide|move_clip|delete_clip|lift|roll)/],
  ['effects', /^(add_effect|apply_effect|set_effect|add_transition|remove_effect|add_speed|set_posterize|add_adjustment)/],
  ['transform', /^(set_property|set_clip_property|set_props?$|transform|set_transform|keyframe|set_keyframe|add_keyframe|scale|rotate|position|set_motion|add_null|set_parent|set_expression)/],
  ['color', /^(grade|color|colour|lut|apply_lut|set_lut|ocio|render_scope|analyze_exposure|wheel|curve|node_|hdr|render_ocio|render_exposure|render_gamut)/],
  ['audio', /^(set_|add_|apply_|auto_|measure_|normalize_|analyze_)?(mix|audio|gain|fader|mute|solo|pan|compress|eq|limiter|limit|loudness|duck|stem|bus|route_)|^track_beats$/],
  ['text', /^(caption|subtitle|add_caption|add_subtitle|edit_caption|transcript_edit|transcri|add_title|title|text|set_text|add_text|set_caption|validate_captions)/],
  ['3d', /^(add_mesh|mesh|model|extrude|bevel|light|camera|particle|emit|simulate|scene|render_3d|add_light|set_light|solve_camera|track|set_camera|add_scene3d|instantiate_gltf|generate_mesh_primitive|contour_sdf_graph|set_mesh_pin|derive_vertex_tangents)/],
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
  if (/^(x|y|pos_x|pos_y)$/i.test(String(param)) && kv.blocks.some(b => b.k && /^position$/i.test(b.k))) return true;   // xfOps wrote Position
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
  if (fam === 'transform' && (ev.phase === 'start' || ev.phase === 'complete')) xfOps(state, MODES, ev);
  if (fam === '3d') sceneOps(state, MODES, ev);
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
      const diff = ev._kvDone || inspectorDiff(state, MODES, ev, fam);   // sceneOps already wrote the inspector
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


// Transform family (set_property/set_transform x y scale rotation opacity, keyframes, bezier ease,
// motion path, motion blur, nulls, parenting, expressions): a gizmo on the viewer — the layer's
// bounding box with 8 handles and its anchor, the previous pose dashed, an arrow when it moved, and a
// readout of the change — plus keyframe diamonds on the clip in the timeline. Geometry comes only from
// the call: x/y are project pixels, placed when the project size is known (session/frame events'
// width/height), otherwise the move is shown in the readout and the box stays centred.
export const GIZMO_MS = 2400;
const XF = { x: 'x', y: 'y', pos_x: 'x', pos_y: 'y', scale: 'scale', rotation: 'rotation', rot_z: 'rotation', rotate: 'rotation', opacity: 'opacity', rot_x: 'rot_x', rot_y: 'rot_y', pos_z: 'pos_z' };
const numOf = v => { if (v == null || typeof v === 'object') return null; const s = String(v).trim(), n = parseFloat(s); return Number.isFinite(n) ? (s.endsWith('%') ? n / 100 : n) : null; };
function xfOps(state, MODES, ev) {
  const P = ev.payload || {}, T = ev.target || {}, A = P.args || (state._xfArgs && state._xfArgs[ev.call_id]) || {};
  if (ev.phase === 'start') { (state._xfArgs || (state._xfArgs = {}))[ev.call_id] = A; }
  const tn = String(ev.tool_name).toLowerCase(), clip = T.clip_id ?? A.clip_id ?? A.clip ?? A.id ?? null;
  const store = state.xform || (state.xform = {}), key = clip ?? '_';
  const cur = store[key] || (store[key] = { x: 0, y: 0, scale: 1, rotation: 0 });
  const prev = { ...cur };
  const g = { t: now(), tool: ev.tool_name, phase: ev.phase, clip, prev, next: cur, read: [] };
  const param = String(T.param ?? P.param ?? A.property ?? A.param ?? '').toLowerCase(), k = XF[param];
  if (ev.phase === 'complete') {
    delete (state._xfArgs || {})[ev.call_id];
    // a single field change (set_property / set_transform param=…, set_keyframe value)
    if (k) {
      const f = numOf(P.from), to = numOf(P.to ?? P.value ?? A.value);
      if (f != null) prev[k] = f;
      if (to != null) cur[k] = to;
      // the inspector's Position row reads x, y together ("0, 0 → 260, 0")
      if ((k === 'x' || k === 'y') && to != null) { const kv = panels(state, MODES, ['kv'])[0], row = kv && (kv.blocks || []).find(b => b.k && /^position$/i.test(b.k)); if (row) { row.from = `${fmtVal(prev.x)}, ${fmtVal(prev.y)}`; row.v = `${fmtVal(cur.x)}, ${fmtVal(cur.y)}`; row._flash = now(); g.posRow = true; } }
      if (to != null) g.read.push(`${param} ${P.from != null ? fmtVal(P.from, param) + ' → ' : ''}${fmtVal(P.to ?? P.value ?? A.value, param)}`);
    }
    // a pose in the args (set_transform {x, y, scale, rotation}, add_null starting pose)
    for (const [ak, sk] of Object.entries(XF)) if (ak !== param && A[ak] != null && numOf(A[ak]) != null && !/keyframe/.test(tn)) { cur[sk] = numOf(A[ak]); g.read.push(`${ak} ${fmtVal(A[ak], ak)}`); }
  }
  // the family's own marks
  if (/add_null/.test(tn)) { g.null = true; g.read.unshift('null'); }
  if (/set_parent/.test(tn)) { const pid = A.parent_id ?? A.parent; g.parent = pid ? store[pid] || { x: 0, y: 0, scale: 1, rotation: 0 } : null; g.read.unshift(pid ? `parent → ${pid}` : 'unparented'); }
  if (/motion_blur/.test(tn)) { const sa = numOf(A.shutter_angle ?? A.shutter ?? A.angle); if (sa != null) { g.blur = sa; g.read.unshift(`shutter ${sa}°`); } }
  if (/expression/.test(tn)) { const op = A.op ?? A.expression ?? A.kind; const ps = ['freq', 'amp', 'mode', 'phase'].filter(x => A[x] != null).map(x => `${x} ${fmtVal(A[x], x)}`).join(', '); if (op) g.read.unshift(`${param ? param + ' = ' : ''}${op}(${ps})`); g.expr = true; }
  if (/bezier/.test(tn)) { const b = A.bezier ?? A.curve ?? A.handles; if (Array.isArray(b) && b.length === 4 && b.every(v => numOf(v) != null)) { g.bez = b.map(numOf); g.read.unshift(`ease [${b.map(v => fmtVal(v)).join(', ')}]`); } }
  if (/motion_path/.test(tn) && Array.isArray(A.points)) { g.path = A.points.filter(p => Array.isArray(p) && p.length >= 2).map(p => [numOf(p[0]), numOf(p[1])]).filter(p => p[0] != null && p[1] != null); g.read.unshift(`motion path · ${g.path.length} points`); }
  // keyframe diamonds on the clip lane (clip-local times)
  if (/keyframe/.test(tn) && ev.phase === 'complete') {
    const times = Array.isArray(A.keys) ? A.keys.map(q => numOf(q && (q.time ?? q.t))).filter(v => v != null) : [numOf(A.time ?? A.t ?? A.at)].filter(v => v != null);
    const tl = state.liveTL || panels(state, MODES, ['timeline'])[0];
    const [tr, i] = clip != null ? findClip(tl, { clip_id: clip, track_id: T.track_id }) : [null, -1];
    if (tr && times.length) { const c = tr.clips[i]; c.keys = c.keys || []; for (const tt of times) if (!c.keys.some(q => Math.abs(q.t - (c.s + tt)) < 1e-6)) c.keys.push({ t: c.s + tt, em: !!g.bez }); else if (g.bez) c.keys.find(q => Math.abs(q.t - (c.s + tt)) < 1e-6).em = true; c._flash = now(); }
    if (!g.read.length || !k) g.read.push(`${times.length} key${times.length === 1 ? '' : 's'}${param ? ' · ' + param : ''}`);
  }
  if (/^(rot_x|rot_y|pos_z)$/.test(k || '') || ['rot_x', 'rot_y', 'pos_z'].some(q => A[q] != null)) g.three = true;
  if (!g.read.length) g.read.push(ev.tool_name);
  state.gizmo = g;   // nothing animates in JS: the viewer draws it while fresh and CSS fades it
}


// 3D / tracking / lighting / mesh family, on the panels of the screen the call opens (m07 tracking,
// m08 modelling, m11 particles, m12 lighting, m13 scene): everything drawn comes from the call's own
// arguments or result — track quads / ROI / mesh grid / point ring / matte seed on the viewer, a light
// glyph at its frame position plus its row in Lights, the mesh op appended to the Stack, a physics
// body per clip, the solve's reprojection error. Nothing is drawn for a value the call did not carry.
const E2 = x => String(x).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const quad8 = c => Array.isArray(c) && c.length >= 8 && c.slice(0, 8).every(v => numOf(v) != null) ? [0, 2, 4, 6].map(i => [numOf(c[i]), numOf(c[i + 1])]) : null;
function modeViews(state, MODES, ev) {
  const m = (state.explicitMode ? null : inferMode(String(ev.tool_name))) || state.mode;   // the screen app.js will show for this call
  return [m, MODES[m] ? (state.views[m] || (state.views[m] = {})) : views(state)];
}
function sceneOps(state, MODES, ev) {
  const P = ev.payload || {}, T = ev.target || {}, tn = String(ev.tool_name).toLowerCase();
  if (ev.phase === 'start') (state._sceneArgs || (state._sceneArgs = {}))[ev.call_id] = P.args || {};
  const A = P.args || (state._sceneArgs && state._sceneArgs[ev.call_id]) || {};
  const R = P.result && typeof P.result === 'object' ? P.result : {};
  const [mode, V] = modeViews(state, MODES, ev);
  const vw = V.viewer || (V.viewer = {});
  const ov = () => (vw.overlay = (vw.overlay || []).filter(o => o._call !== ev.call_id && !(o._kind && o._kind === tn)));
  let hit = false;
  if (ev.phase === 'start' || ev.phase === 'complete') {
    // trackers: the quad / ROI / mesh grid / point / seed outline they were given
    if (/^track_(planar|mesh|features)/.test(tn)) {
      const q = quad8(A.corners) || quad8(A.roi) || (Array.isArray(A.anchors) && A.anchors[0] && quad8(A.anchors[0].corners));
      if (q) { const o = ov(); o.push({ quad: q, grid: /mesh/.test(tn) ? [numOf(A.nx) ?? 7, numOf(A.ny) ?? 5] : null, roi: /features/.test(tn), _call: ev.call_id, _kind: tn }); hit = true; }
    }
    if (tn === 'track_point' && numOf(A.x) != null && numOf(A.y) != null) { const o = ov(); o.push({ pt: [numOf(A.x), numOf(A.y)], r: numOf(A.radius) ?? .12, _call: ev.call_id, _kind: tn }); hit = true; }
    if (tn === 'track_matte' && Array.isArray(A.seeds) && A.seeds[0] && Array.isArray(A.seeds[0].points)) {
      const pts = A.seeds[0].points.map(p => Array.isArray(p) ? [numOf(p[0]), numOf(p[1])] : null).filter(p => p && p[0] != null && p[1] != null);
      if (pts.length >= 3) { const o = ov(); o.push({ poly: pts, _call: ev.call_id, _kind: tn }); hit = true; }
    }
    // lights: glyph at its frame position, row in the Lights list
    if (tn === 'set_light' || tn === 'light_rig') {
      const lists = Object.entries(MODES[mode] ? MODES[mode].panels : {}).filter(([, sp]) => sp.kind === 'list').map(([k]) => V[k] || (V[k] = {}));
      const L = lists[0];
      const add = (id, kind, v, pos, col) => {
        if (L) { L.rows = L.rows || []; let r = L.rows.find(x => x._id === id); if (!r) { r = { _id: id, icon: 'bulb' }; L.rows.push(r); } r.k = id; r.v = v; r.em = true; r._flash = now(); for (const x of L.rows) if (x !== r) x.em = false; }
        if (pos) { const o = ov(); o.push({ light: pos, kind, color: col, label: id, _call: ev.call_id + id, _kind: 'light:' + id }); }
      };
      if (tn === 'set_light') {
        const pos = Array.isArray(A.pos) && numOf(A.pos[0]) != null && numOf(A.pos[1]) != null ? [numOf(A.pos[0]), numOf(A.pos[1])] : null;
        const col = Array.isArray(A.color) && A.color.length >= 3 ? A.color.slice(0, 3).map(numOf) : null;
        const v = [A.kind, numOf(A.intensity) != null ? fmtVal(A.intensity) : null].filter(Boolean).join(' · ');
        if (A.id != null || A.kind != null) { add(String(A.id ?? A.kind), A.kind, v, pos, col); hit = true; }
      } else if (A.preset && ev.phase === 'complete') {
        const pre = String(A.prefix ?? 'rig'), sub = Array.isArray(A.subject) ? A.subject.map(numOf) : null;
        const ids = Array.isArray(R.lights) ? R.lights.map(x => typeof x === 'string' ? x : x && x.id).filter(Boolean) : ['key', 'fill', 'rim'].map(r => `${pre}_${r}`);
        ids.forEach(id => add(id, null, String(A.preset), null, null));
        if (sub && sub[0] != null) { const o = ov(); o.push({ light: sub, kind: 'subject', label: String(A.preset), _call: ev.call_id, _kind: tn }); }
        hit = true;
      }
    }
    // mesh ops: a node on the Stack (modelling), newest in ember
    if (/^(mesh_|generate_mesh_primitive|contour_sdf_graph|set_mesh_pin|derive_vertex_tangents)/.test(tn) && ev.phase === 'complete') {
      const stk = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'stack');
      if (stk) { const S = V[stk[0]] || (V[stk[0]] = {}); S.nodes = S.nodes || []; for (const n of S.nodes) n.em = false; const nm = { mesh_lathe: 'lathe', mesh_subdivide: 'subdivide', mesh_boolean: 'boolean ' + (A.op ?? A.operation ?? 'diff'), mesh_extrude_path: 'extrude path', generate_mesh_primitive: String(A.shape ?? A.primitive ?? A.kind ?? 'primitive'), contour_sdf_graph: 'contour', set_mesh_pin: 'pin', derive_vertex_tangents: 'tangents' }[tn] || tn.replace(/^mesh_/, ''); S.nodes.push({ name: nm, em: true, _flash: now() }); if (S.nodes.length > 7) S.nodes.splice(0, S.nodes.length - 7); hit = true; }
    }
    // camera (m13): orbit ring with the camera on it at its pan, a frustum wedge opening at its fov,
    // a diamond per keyframed pan, the axis triad turned by the pan; scene objects into the Scene tree
    if (/^set_camera/.test(tn) && ev.phase === 'complete') {
      const cam = state.cam || (state.cam = {}), keys = state.camKeys || (state.camKeys = []);
      if (tn === 'set_camera') for (const k of ['cam_x', 'cam_y', 'cam_z', 'cam_pan', 'cam_tilt', 'cam_roll', 'cam_fov']) { if (numOf(A[k]) != null) cam[k] = numOf(A[k]); }
      else if (A.property && numOf(A.value) != null) { cam[A.property] = numOf(A.value); keys.push({ prop: A.property, t: numOf(A.time), v: numOf(A.value) }); if (keys.length > 24) keys.shift(); }
      vw.overlay = (vw.overlay || []).filter(x => x._kind !== 'camera');
      // Keyframes panel: one lane per keyed camera property, diamonds at the keys' timeline times, newest in ember
      const kp = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'keys');
      const timed = keys.filter(k => k.t != null);
      if (kp && timed.length) {
        const KK = V[kp[0]] || (V[kp[0]] = {}), last = timed[timed.length - 1], props = [...new Set(timed.map(k => k.prop))];
        KK.ruler = { t0: 0, t1: Math.max(4, Math.ceil(Math.max(...timed.map(k => k.t)) + 1)), step: 1, fmt: 's', align: 'left' };
        KK.lanes = props.map(pr => ({ label: `<span class=cream>camera</span> <span class=dim>${E2(pr.replace(/^cam_/, ''))}</span>`, html: true, sel: pr === last.prop, keys: timed.filter(k => k.prop === pr).map(k => k === last ? { t: k.t, em: true } : k.t) }));
        KK.playhead = last.t;
      }
      const o = ov(); o.push({ cam: { ...cam }, keys: keys.filter(k => k.prop === 'cam_pan').map(k => k.v), _call: ev.call_id, _kind: 'camera' });
      const kvn = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'kv');
      if (kvn) { ev._kvDone = true; const K = V[kvn[0]] || (V[kvn[0]] = {}); K.blocks = K.blocks || []; for (const [k, lab, u] of [['cam_fov', 'FOV', '°'], ['cam_pan', 'Pan', '°'], ['cam_tilt', 'Tilt', '°'], ['cam_z', 'Dolly', ' px']]) if (cam[k] != null) { let r = K.blocks.find(b => b.k === lab); const v = fmtVal(cam[k]) + u; if (!r) { r = { k: lab }; K.blocks.push(r); } if (r.v !== v) { if (r.v != null) r.from = r.v; r.v = v; r._flash = now(); } } }
      hit = true;
    }
    if (tn === 'add_scene3d' && ev.phase === 'complete' && A.config && typeof A.config === 'object') {
      const tr = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'tree');
      const objs = Array.isArray(A.config.objects) ? A.config.objects : [], lts = Array.isArray(A.config.lights) ? A.config.lights : [];
      if (tr && (objs.length || lts.length)) { const S = V[tr[0]] || (V[tr[0]] = {}); S.items = S.items || []; for (const x of S.items) x.sel = false;
        objs.forEach((ob, i) => S.items.push({ name: String(ob.name ?? ob.id ?? `${ob.kind ?? 'object'} ${i + 1}`), icon: ob.kind === 'plane' ? 'plane' : 'mesh', d: ob.parent ? 1 : 0, sel: i === objs.length - 1, _flash: now() }));
        lts.forEach((l, i) => S.items.push({ name: String(l.name ?? `${l.kind ?? 'light'} ${i + 1}`), icon: 'light' }));
        if (S.items.length > 14) S.items.splice(0, S.items.length - 14); hit = true; }
    }
    // physics bodies: one ring per body at its pixel offset from the canvas centre (needs the project size)
    if (tn === 'simulate_physics' && Array.isArray(A.bodies) && state.session.width) {
      const o = ov(); for (const b of A.bodies) if (b && numOf(b.x) != null && numOf(b.y) != null) o.push({ body: [.5 + numOf(b.x) / state.session.width, .5 + numOf(b.y) / state.session.height], r: (numOf(b.radius_px) ?? 20) / state.session.width, label: b.clip_id, _call: ev.call_id, _kind: tn }); hit = true;
    }
  }
  if (ev.phase === 'complete') {
    // Inspector: the call's own args for what it targeted (light / track / camera / mesh), no placeholders
    const kvn = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'kv');
    if (kvn && !/^set_camera/.test(tn)) {
      const K = V[kvn[0]] || (V[kvn[0]] = {}); K.blocks = K.blocks || [];
      const i = K.blocks.findIndex(b => b._live); if (i >= 0) K.blocks.splice(i);
      const fmtA = v => Array.isArray(v) ? (v.length <= 4 && v.every(x => numOf(x) != null) ? v.map(x => fmtVal(x)).join(', ') : null) : (typeof v === 'object' || v == null ? null : fmtVal(v));
      const lab = k => String(k).replace(/_/g, ' ').replace(/^./, c => c.toUpperCase());
      const rows = Object.entries(A).filter(([k]) => !/^(clip_id|target_clip_id|return_|verts0)/.test(k)).map(([k, v]) => [k, fmtA(v)]).filter(([, v]) => v != null && v !== '').slice(0, 6);
      if (rows.length) { K.blocks.push({ sec: String(A.id ?? A.clip_id ?? ev.tool_name), _live: true }); for (const [k, v] of rows) K.blocks.push({ k: lab(k), v, _live: true, _flash: now() }); hit = true; ev._kvDone = true; }
    }
    // Light rig table: one row per light the calls named, only the columns its args carry
    if (tn === 'set_light' && (A.id != null || A.kind != null)) {
      const rg = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'lightrig');
      if (rg) { const G2 = V[rg[0]] || (V[rg[0]] = {}); G2.rows = G2.rows || []; const id = String(A.id ?? A.kind); let r = G2.rows.find(x => x[0] === id);
        const inten = numOf(A.intensity), temp = numOf(A.temperature ?? A.kelvin);
        if (!r) { r = [id, '', '', 0, false, '']; G2.rows.push(r); }
        r[1] = temp != null ? Math.round(temp) + 'K' : r[1]; if (inten != null) { r[2] = fmtVal(inten); r._i = inten; } if (numOf(A.falloff) != null) r[5] = fmtVal(A.falloff);
        const mx = Math.max(...G2.rows.map(x => x._i || 0)) || 1; for (const x of G2.rows) { x[3] = (x._i || 0) / mx; x[4] = x === r; } hit = true; }
    }
    // Track residual graph: per-frame residuals when the result carries them (else the mockup's empty state)
    const res = [R.frame_rms, R.residuals, R.residuals_px, R.per_frame_rms].find(a => Array.isArray(a) && a.length > 1 && a.every(v => numOf(v) != null));
    if (res && /track|solve/.test(tn)) { const kp = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'keys'); if (kp) { const KK = V[kp[0]] || (V[kp[0]] = {}); const n = res.length, mx = Math.max(...res.map(numOf)); KK.ruler = { t0: 0, t1: n - 1, step: Math.max(1, Math.round(n / 6)), fmt: 'n' }; KK.graph = { pts: res.map(numOf), max: Math.max(1, Math.ceil(mx)), thr: KK.graph && KK.graph.thr }; hit = true; } }
    // a solve / track reports its error: the viewer's left meta line (m07 "solve 0.70 px RMS")
    const rms = numOf(R.rms_px ?? R.rms ?? R.reprojection_rms ?? R.mean_error_px ?? (Array.isArray(R.solves) && R.solves[0] ? R.solves[0].rms_px : null));
    if (rms != null && /solve|track/.test(tn)) { vw.left = `${/solve/.test(tn) ? 'solve' : 'track'} ${rms.toFixed(2)} px RMS`; hit = true; }
    if (tn === 'particle_simulate_raw') { const L = Object.entries(MODES[mode] ? MODES[mode].panels : {}).find(([, sp]) => sp.kind === 'list'); const n = numOf(A.steps); if (L && n != null) { const X2 = V[L[0]] || (V[L[0]] = {}); X2.rows = X2.rows || []; let r = X2.rows.find(x => x._id === 'raw'); if (!r) { r = { _id: 'raw', k: 'raw sim' }; X2.rows.push(r); } r.v = `${fmtVal(n)} steps`; r._flash = now(); hit = true; } }
    delete (state._sceneArgs || {})[ev.call_id];
  }
  return hit;
}
