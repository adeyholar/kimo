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

export const FAMILIES = [
  ['ingest', /^(import|ingest|add_media|relink|load_media|add_clip|add_track|insert)/],
  ['edit', /^(cut|split|blade|razor|trim|extend|ripple|slip|slide|move_clip|delete_clip|lift|roll)/],
  ['effects', /^(add_effect|apply_effect|set_effect|add_transition|remove_effect|add_speed|set_posterize|add_adjustment)/],
  ['transform', /^(transform|set_transform|keyframe|set_keyframe|add_keyframe|scale|rotate|position|set_motion|add_null|set_parent|set_expression)/],
  ['color', /^(grade|color|colour|lut|apply_lut|set_lut|ocio|render_scope|analyze_exposure|wheel|curve|node_|hdr|render_ocio|render_exposure|render_gamut)/],
  ['audio', /^(mix|audio|gain|fader|set_gain|set_fader|mute|solo|pan|compress|eq|limiter|loudness|duck|stem|bus|route_)/],
  ['text', /^(caption|subtitle|add_caption|add_subtitle|edit_caption|transcript_edit|transcri|add_title|title|text|set_text|add_text|set_caption|validate_captions)/],
  ['3d', /^(add_mesh|mesh|model|extrude|bevel|light|camera|particle|emit|simulate|scene|render_3d|add_light|set_light|solve_camera|track)/],
  ['export', /^(export|render|queue_render|encode|deliver)/],
];
export function family(name) {
  const n = String(name || '').toLowerCase();
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
  const rows = Object.entries(P).filter(([k, v]) => v != null && typeof v !== 'object').slice(0, 3);
  if (param != null && !rows.length && (P.to ?? P.value) != null) rows.push([param, P.to ?? P.value]);
  if (!rows.length) return false;
  kv.blocks.push({ sec: ev.tool_name, _live: true });
  for (const [k, v] of rows) kv.blocks.push({ k: label(k), v: fmtVal(v, k), _live: true, _flash: now() });
  return true;
}

function hud(state, ev, fam, text) {
  state.hud = { fam, tool: ev.tool_name, text: text || ev.tool_name, phase: ev.phase, t: now() };
}

// Timeline effects of edit / ingest / effects / audio / text tools.
function timelineOps(state, MODES, ev, fam) {
  const tl = panels(state, MODES, ['timeline'])[0]; if (!tl || !tl.tracks) return false;
  const P = ev.payload || {}, T = ev.target || {}, n = String(ev.tool_name).toLowerCase();
  const ph = P.at ?? tl.playhead;
  if (fam === 'ingest' && (P.s != null || P.clip || n.startsWith('add_track'))) {
    let t = findTrack(tl, T.track_id || P.track);
    if (!t) { t = { label: T.track_id || P.track || 'V' + (tl.tracks.length + 1), clips: [] }; tl.tracks.unshift(t); }
    if (P.s != null) {
      t.clips = t.clips || []; const s = +P.s, e = +(P.e ?? s + 4);
      // overwrite edit: whatever sat under the new clip is trimmed back to it
      t.clips = t.clips.filter(c => !(c.s >= s && c.e <= e));
      for (const c of t.clips) { if (c.s < s && c.e > s) c.e = s; else if (c.s < e && c.e > e) c.s = e; }
      t.clips.push({ id: T.clip_id || P.clip, label: P.label || T.clip_id || P.clip, s: +P.s, e: +(P.e ?? +P.s + 4), thumb: P.thumb, _flash: now() }); }
    t._flash = now(); return true;
  }
  const [t, i] = findClip(tl, T);
  if (fam === 'edit' && t) {
    const c = t.clips[i];
    if (/^(cut|split|blade|razor)/.test(n) && ph > c.s && ph < c.e) {
      const right = { ...c, id: (c.id || c.label) + '_b', s: ph, _flash: now() };
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
  if (fam === 'effects' && t) {
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

// Unified event -> legacy rail events + family effects. Returns the legacy
// events the reducer should apply (the rail), having already mutated views.
export function route(ev, state, MODES) {
  if (!ev || ev.type || !ev.tool_name) return [ev];        // legacy or non-tool event: unchanged
  const fam = family(ev.tool_name), id = ev.call_id ?? ev.tool_name + '@' + (ev.ts ?? '');
  const out = [];
  const T = ev.target || {}, P = ev.payload || {};
  const param = T.param || P.param, val = P.to ?? P.value;
  const args = { ...(T.clip_id ? { clip: T.clip_id } : {}), ...(param && val != null ? { [param]: fmtVal(val, param) } : {}) };
  if (ev.phase === 'start') {
    out.push({ type: 'tool_start', id, name: ev.tool_name, args: P.args ? P.args : (Object.keys(args).length ? args : undefined) });
    hud(state, ev, fam);
  } else if (ev.phase === 'progress') {
    const pct = P.pct ?? (P.done != null && P.total ? 100 * P.done / P.total : null);
    out.push({ type: 'tool_progress', id, progress: P.done != null ? { done: P.done, total: P.total } : { pct } });
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
      const hit = timelineOps(state, MODES, ev, fam);
      const diff = inspectorDiff(state, MODES, ev, fam);
      if (!hit && !diff) hud(state, ev, fam, ev.tool_name + ' ✓'); else hud(state, ev, fam, ev.tool_name + ' ✓');
    } else hud(state, ev, fam, ev.tool_name + ' failed');
  }
  return out;
}
