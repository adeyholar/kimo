// COV-C: dedicated visuals for the colour-pipeline, project / history / template / cache and frame-QA tools.
// Every reaction lands in an element the mockups already have:
//   pipeline (m05 Color pipeline list): input / working / view / output rows, a clip's input override row,
//     the HDR rows (MaxCLL / MaxFALL / Peak), each changed row from -> to;
//   project (m02): the History list (checkpoints, restore, undo / redo, save, load), the Diff list
//     (project_diff, template_define / template_delete), the Batch table (batch, recipe_apply,
//     template_apply), the Media list + its footer (load_project dependencies, save_project);
//     set_meta renames the clip / track on the timeline and writes Name / Role / Tags in the Inspector (m16);
//   qa: diff_frames, lint_frame and the frame cache answer in the HUD status toast and keep the result on their
//     own rail row (Naman 7:24), no mode hop, nothing on the canvas.
// Never a raw id: clips read as their timeline names, checkpoints and templates by the names the call gave.
import { _now as now, RAW_ID, findClip, findTrack, timelineView, numOf } from './router.js';

export const COV_C = {
  pipeline: /^(set_color_pipeline|set_clip_input_color|set_hdr_mastering|set_ocio_view_delivery)$/,
  project: /^(set_meta|load_project|save_project|project_diff|checkpoint|checkpoint_delete|restore|undo|redo|batch|recipe_apply|template_apply|template_define|template_delete)$/,
  qa: /^(diff_frames|lint_frame|cache_clear|cache_set_budget)$/,
};

// Kimo colour IDs (kimo-core color: gamut 0..7, transfer 0..8)
const GAMUT = ['Rec.709', 'Rec.2020', 'P3-D65', 'AP1', 'AP0', 'AWG3', 'SGamut3', 'SGamut3.Cine'];   /* short: the list is 258 px wide */
const TRANSFER = ['linear', 'sRGB', 'gamma 2.4', 'ACEScct', 'LogC3', 'S-Log3', 'PQ', 'HLG', 'BT.709'];
export function space(g, t) {
  g = numOf(g); t = numOf(t);
  if (g == null && t == null) return null;
  const G = GAMUT[g] ?? null, T = TRANSFER[t] ?? null;
  if (g === 3 && t === 0) return 'ACEScg';
  if (g === 3 && t === 3) return 'ACEScct';
  if (g === 0 && (t === 1 || t === 8)) return 'Rec.709 ' + T;
  if (g === 1 && (t === 6 || t === 7)) return 'Rec.2100 ' + T;
  if (g === 5 && t === 4) return 'ARRI LogC3';
  if (g === 6 && t === 5) return 'S-Log3';
  if (g === 7 && t === 5) return 'S-Log3 Cine';
  return [G, T].filter(Boolean).join(' ') || null;
}
const working = g => { g = numOf(g); return g == null ? null : g === 3 ? 'ACEScg' : (GAMUT[g] ?? 'gamut ' + g) + ' linear'; };
const nits = v => { const n = numOf(v); return n == null ? null : (Math.round(n * 1000) / 1000) + ' nits'; };
const base = p => String(p ?? '').split(/[\\/]/).pop();
const bytes = n => { n = numOf(n); if (n == null) return null; const u = ['B', 'KB', 'MB', 'GB']; let i = 0; while (n >= 1024 && i < 3) { n /= 1024; i++; } return (i ? Math.round(n * 10) / 10 : Math.round(n)) + ' ' + u[i]; };
const clean = s => (s != null && s !== '' && !RAW_ID.test(String(s)) ? String(s) : null);
function panel(state, MODES, mode, kind) {
  const M = MODES[mode]; if (!M) return null;
  const e = Object.entries(M.panels).find(([, sp]) => sp.kind === kind); if (!e) return null;
  const V = state.views[mode] || (state.views[mode] = {});
  return V[e[0]] || (V[e[0]] = {});
}
const clipName = (state, MODES, id) => {
  if (id == null) return null;
  const tl = timelineView(state, MODES); const s = String(id);
  if (tl) { const [t, i] = findClip(tl, { clip_id: s.replace(/^@/, '') }); if (t) return clean(t.clips[i].label); }
  return s.startsWith('@') ? s.slice(1) : clean(s);
};

// ---------- pipeline: the m05 Color pipeline list ----------
function pipeRows(state, MODES) {
  const kv = panel(state, MODES, 'color', 'kv'); if (!kv) return;
  const S = state._pipe, ch = S._ch || {}, t = now(), row = (k, v, key) => {   /* from -> to when both fit the 258 px row, else the new value flashes alone */
    const c = ch[key], fits = c && c.from != null && c.from !== v && String(k).length + String(c.from).length + String(v ?? '').length <= 23;
    return { k, v: v ?? '—', ...(c ? { _flash: t, ...(fits ? { from: c.from } : {}) } : {}), ...(c && c.dim ? { dim: true } : {}) };
  };
  const B = [row('input', S.input, 'input')];
  for (const [nm, v] of Object.entries(S.clips || {}).slice(-2)) B.push(row(nm, v, 'clip:' + nm));
  B.push(row('working', S.working, 'working'));
  if (S.view != null) B.push(S._viewErr ? { ...row('view', S.view, 'view'), dim: true } : { ...row('view', S.view, 'view'), emb: true, bar: true });
  B.push(row('output', S.output, 'output'));
  if (S.hdr) {
    B.push({ sec: 'HDR' });
    for (const [k, key] of [['MaxCLL', 'max_cll'], ['MaxFALL', 'max_fall'], ['Peak', 'max_nits'], ['Min', 'min_nits']]) B.push({ ...row(k, S.hdr[key], 'hdr:' + key), h: 34 });
  }
  kv.blocks = B; S._ch = {};
}
function setPipe(S, key, v) { const prev = key.startsWith('clip:') ? (S.clips || {})[key.slice(5)] : key.startsWith('hdr:') ? (S.hdr || {})[key.slice(4)] : S[key]; if (v == null) return; (S._ch || (S._ch = {}))[key] = { from: prev ?? null }; if (key.startsWith('clip:')) (S.clips || (S.clips = {}))[key.slice(5)] = v; else if (key.startsWith('hdr:')) (S.hdr || (S.hdr = {}))[key.slice(4)] = v; else S[key] = v; }
export function pipelineOps(state, MODES, ev) {
  const tn = String(ev.tool_name), P = ev.payload || {};
  if (ev.phase === 'start') { (state._covArgs || (state._covArgs = {}))[ev.call_id] = P.args || {}; return false; }
  if (ev.phase !== 'complete' && ev.phase !== 'error') return false;
  const A = (state._covArgs && state._covArgs[ev.call_id]) || P.args || {}; if (state._covArgs) delete state._covArgs[ev.call_id];
  const R = P.result && typeof P.result === 'object' ? P.result : {}, err = ev.phase === 'error';
  const S = state._pipe || (state._pipe = { input: null, working: null, view: null, output: null });
  if (tn === 'set_color_pipeline') {
    if (err) return false;
    const st = (R.settings && typeof R.settings === 'object' ? R.settings : null) || A.settings || {};
    if (st.enabled === false) { setPipe(S, 'working', 'off (display-referred)'); }
    else {
      setPipe(S, 'input', space(st.input_gamut, st.input_transfer));
      setPipe(S, 'working', working(st.working_gamut));
      setPipe(S, 'output', space(st.output_gamut, st.output_transfer));
    }
    if (st.mastering && typeof st.mastering === 'object') for (const k of ['max_cll', 'max_fall', 'max_nits', 'min_nits']) setPipe(S, 'hdr:' + k, nits(st.mastering[k]));
  } else if (tn === 'set_clip_input_color') {
    if (err) return false;
    const T = ev.target || {}, cn = clipName(state, MODES, A.clip_id ?? T.clip_id), nm = cn ? cn.replace(/\.[a-z0-9]{2,4}$/i, '').slice(0, 12) : 'clip input';
    const v = space(R.gamut ?? A.gamut, R.transfer ?? A.transfer); if (!v) return false;
    if (!(S.clips && S.clips[nm] != null)) { (S._ch || (S._ch = {}))['clip:' + nm] = { from: S.input ?? 'project input' }; (S.clips || (S.clips = {}))[nm] = v; } else setPipe(S, 'clip:' + nm, v);
  } else if (tn === 'set_hdr_mastering') {
    if (err) return false;
    const m = (R.mastering && typeof R.mastering === 'object' ? R.mastering : null) || A.metadata || {};
    if (A.clear || R.mastering === null) { for (const k of ['max_cll', 'max_fall', 'max_nits', 'min_nits']) if (S.hdr) setPipe(S, 'hdr:' + k, 'cleared'); if (!S.hdr) return false; }
    else for (const k of ['max_cll', 'max_fall', 'max_nits', 'min_nits']) setPipe(S, 'hdr:' + k, nits(m[k]));
  } else if (tn === 'set_ocio_view_delivery') {
    const sel = A.selection || {}, view = typeof sel.view === 'string' && sel.view ? sel.view : (typeof sel.display === 'string' && sel.display ? sel.display : null);
    if (!view) return false;
    if (err) { (S._ch || (S._ch = {})).view = { from: null, dim: true }; S.view = '✗ ' + view; S._viewErr = true; }
    else {
      if (S._viewErr) { S.view = null; S._viewErr = false; } setPipe(S, 'view', view);
      const og = numOf(sel.output_gamut), out = space(og === 1 ? 1 : og === 0 ? 0 : null, sel.output_transfer); if (out) setPipe(S, 'output', out);
    }
  } else return false;
  pipeRows(state, MODES); ev._kvDone = true; return true;
}

// ---------- project (m02 History / Diff / Batch / Media, m16 timeline + Inspector for set_meta) ----------
function agoAll(items, t) {
  for (const it of items) { const s = (t - it._t) / 1000; it.ago = s < 60 ? 'now' : s < 3600 ? Math.floor(s / 60) + 'm' : Math.floor(s / 3600) + 'h'; it.live = false; }
}
function histPush(state, MODES, item, opts = {}) {
  const H = panel(state, MODES, 'batch', 'history'); if (!H) return false;
  const t = now(); let items = (H.items || []).filter(x => !x.dim);   /* a row shown as deleted leaves on the next change */
  if (opts.reset) items = [];
  if (opts.replace) items = items.filter(x => !(x._cp && x.id === item.id));
  if (item._step && items[0] && items[0]._step) items.shift();   /* undo / redo: one row is the stack position, the latest call wins */
  item = { ...item, _t: t, _flash: t };
  items.unshift(item); agoAll(items, t); item.live = true;
  for (const x of items) x.sel = x === (opts.sel || item);
  const src = (state.views.batch && state.views.batch.viewer && state.views.batch.viewer.src) || null; if (src && !item.src) item.src = src;
  while (items.length > 5) { let j = -1; for (let i = items.length - 1; i > 0; i--) if (!items[i]._cp) { j = i; break; } items.splice(j > 0 ? j : items.length - 1, 1); }   /* checkpoints outlive step rows */
  H.items = items; return true;
}
const rev = s => { s = String(s ?? ''); let m;
  if (s === 'current' || s === '') return 'now';
  if ((m = /^checkpoint:(.+)$/.exec(s))) return m[1];
  if ((m = /^undo:(\d+)$/.exec(s))) return +m[1] === 1 ? '1 step back' : m[1] + ' steps back';
  if ((m = /^file:(.+)$/.exec(s))) return base(m[1]);
  return clean(s) || 'revision'; };
function diffLines(state, MODES, ctx, lines, meta) {
  const D = panel(state, MODES, 'batch', 'diffbatch'); if (!D) return false;
  const t = now();
  if (D._ctx !== ctx) { D.diff = []; D._ctx = ctx; D.meta = ''; }
  D.diff = (D.diff || []).concat(lines).slice(-6); D.titleSmall = ctx; if (meta != null) D.meta = meta; D._flash = t; return true;
}
function batchTable(state, MODES, title, meta, rows) {
  const D = panel(state, MODES, 'batch', 'diffbatch'); if (!D) return false;
  D.batch = { title, meta, rows: rows.slice(0, 6) }; D._flash = now(); return true;
}
function mediaFoot(state, MODES, foot, items) {
  const M = panel(state, MODES, 'batch', 'media'); if (!M) return false;
  M.foot = foot; M._footFlash = now(); if (items) M.items = items; return true;
}
export function projectOps(state, MODES, ev) {
  const tn = String(ev.tool_name), P = ev.payload || {};
  if (ev.phase === 'start') {
    (state._covArgs || (state._covArgs = {}))[ev.call_id] = P.args || {};
    if (tn === 'set_meta' && P.args && typeof P.args.id === 'string') {   /* the rail's copy: the id reads as the clip / track's name, else is left out */
      const A0 = P.args, id = A0.id.replace(/^@/, ''), tl = timelineView(state, MODES);
      const lb = A0.target === 'track' ? (tl && findTrack(tl, id) ? clean(findTrack(tl, id).label) : null) : clipName(state, MODES, A0.id);
      const { id: _, ...rest } = A0; ev.payload = { ...P, args: lb ? { ...rest, [A0.target === 'track' ? 'track' : 'clip']: lb } : rest };
    }
    return false;
  }
  if (ev.phase !== 'complete' && ev.phase !== 'error') return false;
  const A = (state._covArgs && state._covArgs[ev.call_id]) || P.args || {}; if (state._covArgs) delete state._covArgs[ev.call_id];
  const R = P.result && typeof P.result === 'object' ? P.result : {}, err = ev.phase === 'error';
  const n = v => Math.round(numOf(v) ?? 0), pl = (k, w) => `${k} ${w}${k === 1 ? '' : 's'}`;
  let hit = false;
  if (err) {
    if (/^(batch|recipe_apply|template_apply)$/.test(tn)) {
      const name = tn === 'batch' ? (clean(A.label) || 'Batch') : `${tn === 'recipe_apply' ? 'Recipe' : 'Template'} ${A.name ?? ''}`.trim();
      hit = batchTable(state, MODES, name, P.code === 'batch_failed' ? 'failed · rolled back' : 'failed', []);
    }
    ev._kvDone = hit; return hit;
  }
  switch (tn) {
    case 'checkpoint': {
      const name = R.name ?? A.name; if (!name) break;
      const H = panel(state, MODES, 'batch', 'history'), was = H && (H.items || []).some(x => x._cp && x.id === name);
      hit = histPush(state, MODES, { id: String(name), name: was ? 'checkpoint · replaced' : 'checkpoint', _cp: true }, { replace: true }); break;
    }
    case 'checkpoint_delete': {
      const H = panel(state, MODES, 'batch', 'history'); if (!H) break;
      const it = (H.items || []).find(x => x._cp && x.id === A.name);
      if (it) { it.name = 'deleted'; it.dim = true; it.live = false; it._cp = false; it._flash = now(); hit = true; }
      else hit = histPush(state, MODES, { id: String(A.name ?? 'checkpoint'), name: 'deleted', dim: true });
      if (it && it.sel) { it.sel = false; const nx = H.items.find(x => !x.dim); if (nx) { nx.sel = true; nx.live = true; } }
      break;
    }
    case 'restore': {
      const name = R.restored ?? A.name; if (!name) break;
      const H = panel(state, MODES, 'batch', 'history'), cp = H && (H.items || []).find(x => x._cp && x.id === name);
      hit = histPush(state, MODES, { id: 'restore', name: String(name), src: cp && cp.src }, { sel: cp });
      if (cp) { cp._flash = now(); } ev._hudText = `restore · back to ${name}`; break;
    }
    case 'undo': case 'redo': {
      const k = n(R[tn === 'undo' ? 'undone' : 'redone'] ?? A.steps ?? 1);
      hit = histPush(state, MODES, { _step: true, id: k > 1 ? `${tn} ${k} steps` : tn, name: `${n(R.undo_left)} left` });   /* the History column fits ~13 characters */
      ev._hudText = `${tn} ${pl(k, 'step')} · ${n(R.undo_left)} left to undo, ${n(R.redo_available)} to redo`; break;
    }
    case 'save_project': {
      const f = base(R.path ?? A.path) || 'project';
      const hist = R.history != null || A.with_history;
      hit = histPush(state, MODES, { id: f, name: 'saved' });
      ev._hudText = `save_project · ${f}${hist ? ` + history: ${pl(n(R.undo_steps), 'undo step')}, ${pl(n(R.checkpoints), 'checkpoint')}` : ''}`;
      mediaFoot(state, MODES, `save_project: ${f}${hist && R.history_bytes != null ? `, history ${bytes(R.history_bytes)}` : ''}`); break;
    }
    case 'load_project': {
      const f = base(A.path) || 'project';
      hit = histPush(state, MODES, { id: f, name: 'loaded' }, { reset: true });
      ev._hudText = `load_project · ${f}${R.undo_steps != null ? `: ${pl(n(R.undo_steps), 'undo step')}, ${pl(n(R.checkpoints), 'checkpoint')}` : ''}`;
      const D = R.dependencies && typeof R.dependencies === 'object' ? R.dependencies : null;
      if (D) {
        const bad = (Array.isArray(D.not_ok) ? D.not_ok : []).filter(x => x && typeof x === 'object');
        const by = new Map(); for (const x of bad) { const nm = base(x.path ?? x.resolved) || String(x.kind || 'file'), k = nm + '|' + (x.kind || '') + '|' + (x.status || ''); const g = by.get(k) || { name: nm, kind: x.kind, status: x.status || 'missing', n: 0 }; g.n++; by.set(k, g); }
        const items = [...by.values()].slice(0, 4).map(g => ({ name: g.name, note: `${g.kind ? g.kind + ' ' : ''}${g.status}${g.n > 1 ? ' ×' + g.n : ''}`, em: true }));
        mediaFoot(state, MODES, `load_project: ${pl(n(D.total), 'file')}, ${n(D.missing)} missing${n(D.changed) ? `, ${n(D.changed)} changed` : ''}`, items.length ? items : undefined);
      } else mediaFoot(state, MODES, `load_project: ${f}, ${pl(n(R.tracks), 'track')}`);
      break;
    }
    case 'project_diff': {
      const ctx = `${rev(A.a)} to ${rev(A.b ?? 'current')}`, L = [];
      const nm = x => (x && typeof x === 'object' ? clean(x.name) || clipName(state, MODES, x.id) : clean(x));
      for (const x of (Array.isArray(R.added) ? R.added : [])) { const s = nm(x); if (s) L.push(['+', s]); }
      for (const x of (Array.isArray(R.removed) ? R.removed : [])) { const s = nm(x); if (s) L.push(['-', s]); }
      for (const x of (Array.isArray(R.changed) ? R.changed : [])) { const s = nm(x); const k = Array.isArray(x && x.changes) ? x.changes.length : 0; if (s) L.push(['~', k ? `${s}  ${pl(k, 'change')}` : s]); }
      const T = R.tracks && typeof R.tracks === 'object' ? R.tracks : {};
      for (const x of (Array.isArray(T.renamed) ? T.renamed : [])) if (x && clean(x.after)) L.push(['~', `track ${clean(x.before) || 'unnamed'} -> ${x.after}`]);
      for (const x of (Array.isArray(T.added) ? T.added : [])) { const s = nm(x); if (s) L.push(['+', 'track ' + s]); }
      for (const x of (Array.isArray(T.removed) ? T.removed : [])) { const s = nm(x); if (s) L.push(['-', 'track ' + s]); }
      if (R.identical || !L.length) L.push(['~', R.identical ? 'identical' : 'no named changes']);
      const C = R.counts || {};
      const D = panel(state, MODES, 'batch', 'diffbatch'); if (D) D._ctx = null;
      hit = diffLines(state, MODES, ctx, L, `+${n(C.added)} ~${n(C.changed)} -${n(C.removed)}`); break;
    }
    case 'template_define': {
      const name = R.name ?? A.name; if (!name && !A.library_path) break;
      const ps = Array.isArray(R.params) ? R.params : (Array.isArray(A.params) ? A.params.map(p => p && p.name).filter(Boolean) : []);
      const ops = n(R.op_count ?? (Array.isArray(A.ops) ? A.ops.length : 0));
      hit = diffLines(state, MODES, 'templates', [[R.replaced ? '~' : '+', `template ${name ?? base(A.library_path)}${ps.length ? ` (${ps.join(', ')})` : ''} · ${pl(ops, 'op')}`]]); break;
    }
    case 'template_delete': {
      const name = R.deleted ?? A.name; if (!name) break;
      hit = diffLines(state, MODES, 'templates', [['-', `template ${name}`]]); break;
    }
    case 'batch': {
      const ops = Array.isArray(A.ops) ? A.ops : [], res = Array.isArray(R.results) ? R.results.filter(x => x && typeof x === 'object') : [];
      const warn = new Set((Array.isArray(R.warnings) ? R.warnings : []).map(w => w && w.index));
      const rows = (res.length ? res : ops.map((o, i) => ({ index: i, tool: o && o.tool }))).map(x => {
        const op = ops[x.index] || {}, a = op.args || {};
        const who = clipName(state, MODES, a.clip_id) || clean(a.name) || clean(a.track_id) || `op ${(x.index ?? 0) + 1}`;
        return [who, String(x.tool ?? op.tool ?? ''), R.dry_run ? 'dry run' : warn.has(x.index) ? 'warned' : 'done'];
      });
      hit = batchTable(state, MODES, clean(A.label) ? 'Batch ' + A.label : 'Batch', `${n(R.applied ?? rows.length)} / ${ops.length || rows.length}`, rows); break;
    }
    case 'recipe_apply': {
      const name = (R.recipe && R.recipe.name) || A.name || 'recipe';
      const roles = R.roles && typeof R.roles === 'object' ? Object.keys(R.roles) : [];
      const rows = roles.map(k => [k.split('/').pop(), String(name), R.dry_run ? 'dry run' : 'added']);
      hit = batchTable(state, MODES, 'Recipe ' + name, `${pl(n(R.applied), 'op')}`, rows.length ? rows : [[String(name), 'recipe', R.dry_run ? 'dry run' : 'done']]); break;
    }
    case 'template_apply': {
      const name = R.template ?? A.name ?? 'template';
      const created = Array.isArray(R.created) ? R.created : (R.created && Array.isArray(R.created.clip_ids) ? R.created.clip_ids : []);
      const label = clean(P.label) || (R.args && typeof R.args === 'object' ? Object.values(R.args).find(v => typeof v === 'string' && clean(v)) : null);
      const rows = created.map((c, i) => [clipName(state, MODES, c) || (i === 0 && label) || `clip ${i + 1}`, String(name), R.dry_run ? 'dry run' : 'added']);
      hit = batchTable(state, MODES, 'Template ' + name, pl(n(R.applied_ops ?? R.count), 'op'), rows.length ? rows : [[label || String(name), String(name), R.dry_run ? 'dry run' : 'done']]); break;
    }
    case 'set_meta': hit = metaOps(state, MODES, A, R); break;
  }
  if (hit) ev._kvDone = true;
  return hit;
}
function metaOps(state, MODES, A, R) {
  const tl = timelineView(state, MODES); const t = now();
  const target = R.target ?? A.target ?? 'clip', id = String(R.id ?? A.id ?? '').replace(/^@/, '');
  const name = R.name !== undefined ? R.name : A.name;
  let hit = false, from = null, nm = null;
  if (target === 'track') {
    const tr = tl && (findTrack(tl, id) || (tl.tracks || []).find(x => x.name === id)); if (!tr) return false;
    from = clean(tr.label); if (typeof name === 'string' && name) { tr.label = name.slice(0, 16); tr.name = name; } tr._flash = t; nm = tr.label; hit = true;
  } else {
    let [tr, i] = tl ? findClip(tl, { clip_id: id }) : [null, -1];
    if (!tr && tl) for (const x of tl.tracks || []) { const j = (x.clips || []).findIndex(c => c.name === id); if (j >= 0) { tr = x; i = j; break; } }
    if (!tr) return false;
    const c = tr.clips[i]; from = clean(c.label);
    if (typeof name === 'string' && name) { c.label = name; c.name = name; } c._flash = t; nm = clean(c.label); hit = true;
  }
  const kvE = Object.entries(MODES.edit ? MODES.edit.panels : {}).find(([, sp]) => sp.kind === 'kv');
  if (kvE) {
    const V = state.views.edit || (state.views.edit = {}), kv = V[kvE[0]] || (V[kvE[0]] = {});
    kv.blocks = (kv.blocks || []); const j = kv.blocks.findIndex(b => b._live); if (j >= 0) kv.blocks.splice(j);
    if (nm) kv.sub = nm;
    kv.blocks.push({ sec: target === 'track' ? 'set_meta · track' : 'set_meta', _live: true });
    if (typeof name === 'string' && name) kv.blocks.push({ k: 'Name', v: name, ...(from && from !== name ? { from } : {}), _live: true, _flash: t });
    else if (name === null) kv.blocks.push({ k: 'Name', v: 'cleared', _live: true, _flash: t });
    const role = R.role !== undefined ? R.role : A.role; if (role !== undefined) kv.blocks.push({ k: 'Role', v: role == null ? 'cleared' : String(role), _live: true, _flash: t });
    const tags = Array.isArray(R.tags) ? R.tags : Array.isArray(A.tags) ? A.tags : null;
    if (tags) kv.blocks.push({ k: 'Tags', v: tags.length ? tags.join(', ') : 'none', _live: true, _flash: t });
    else { const add = Array.isArray(A.add_tags) ? A.add_tags.map(x => '+' + x) : [], rm = Array.isArray(A.remove_tags) ? A.remove_tags.map(x => '-' + x) : []; if (add.length || rm.length) kv.blocks.push({ k: 'Tags', v: [...add, ...rm].join(' '), _live: true, _flash: t }); }
  }
  return hit;
}

// ---------- qa / cache: the HUD status toast and the call's own rail row (Naman 7:24: no viewer or layout change) ----------
// diff_frames / lint_frame read, cache_clear / cache_set_budget tune memory: nothing on the canvas moves and the workspace
// does not hop. The toast says what came back ("diff_frames · now vs graded: 77% changed, major") and the rail row keeps it
// in place of the start-time args, so the result stays readable after the toast fades.
const sec = v => { const x = numOf(v); return x == null ? '' : ` ${Math.round(x * 100) / 100}s`; };
function side(state, MODES, s) {
  if (!s || typeof s !== 'object') return 'now';
  const src = String(s.source ?? 'composite'); let m, w;
  if (src === 'composite') w = 'now';
  else if ((m = /^snapshot:(.+)$/.exec(src))) w = rev(m[1]);
  else if ((m = /^clip:(.+)$/.exec(src))) w = clipName(state, MODES, m[1]) || 'clip';
  else if ((m = /^file:(.+)$/.exec(src))) w = base(m[1]);
  else w = rev(src);
  return w + (s.time != null ? sec(s.time) : s.frame != null ? ` f${Math.round(numOf(s.frame))}` : '');
}
const LINT = { out_of_bounds: 'out of bounds', invisible_layer: 'invisible layer', low_res_upscale: 'low-res upscale', static_frame: 'static frame' };
export function qaOps(state, MODES, ev) {
  const tn = String(ev.tool_name), P = ev.payload || {};
  if (ev.phase === 'start') { (state._covArgs || (state._covArgs = {}))[ev.call_id] = P.args || {}; return false; }
  if (ev.phase !== 'complete') return false;
  const A = (state._covArgs && state._covArgs[ev.call_id]) || P.args || {}; if (state._covArgs) delete state._covArgs[ev.call_id];
  const R = P.result && typeof P.result === 'object' ? P.result : {};
  const n = v => Math.round(numOf(v) ?? 0), pl = (k, w) => `${k} ${w}${k === 1 ? '' : 's'}`;
  let rail = null, toast = null;
  if (tn === 'diff_frames') {
    const a = side(state, MODES, R.a || A.a), b = side(state, MODES, R.b || A.b), D = R.delta || {}, pct = numOf(D.changed_percent);
    const what = D.bit_identical ? 'identical' : pct != null ? `${pct < 1 ? Math.round(pct * 100) / 100 : Math.round(pct * 10) / 10}% changed${R.verdict_hint ? ', ' + R.verdict_hint : ''}` : null;
    if (!what) return false;
    const psnr = numOf((R.metrics || {}).psnr_db);
    rail = { changed: D.bit_identical ? 'none' : what.replace(/ changed/, '') }; toast = `${tn} · ${a} vs ${b}: ${what}${psnr != null && !D.bit_identical ? ` (PSNR ${Math.round(psnr * 10) / 10} dB)` : ''}`;
  } else if (tn === 'lint_frame') {
    const t = A.time != null ? sec(A.time).trim() : Array.isArray(A.times) ? pl(A.times.length, 'time') : '';
    const S = R.summary || {}, by = S.counts_by_code && typeof S.counts_by_code === 'object' ? S.counts_by_code : {};
    const parts = Object.entries(by).filter(([, k]) => numOf(k)).map(([c, k]) => `${LINT[c] || c.replace(/_/g, ' ')} ×${n(k)}`);
    const what = parts.length ? parts.join(', ') : 'clean';
    rail = { [t || 'frame']: what }; toast = `${tn}${t ? ' ' + t : ''} · ${what}`;
  } else if (tn === 'cache_set_budget') {
    const cap = numOf(R.cap_bytes) ?? (numOf(A.megabytes) != null ? numOf(A.megabytes) * 1048576 : null); if (cap == null) return false;
    const prev = state._cacheCap; state._cacheCap = cap;
    const cv = cap === 0 ? 'off' : bytes(cap), pv = prev != null && prev !== cap ? (prev === 0 ? 'off' : bytes(prev)) + ' → ' : '';
    rail = { budget: pv + cv }; toast = `frame cache ${pv}${cv}, ${bytes(R.bytes ?? 0)} used`;
  } else if (tn === 'cache_clear') {
    rail = { freed: `${bytes(R.freed_bytes ?? 0)}, ${pl(n(R.cleared_entries), 'frame')}` }; toast = `frame cache cleared · ${pl(n(R.cleared_entries), 'frame')}, ${bytes(R.freed_bytes ?? 0)} freed`;
  } else return false;
  ev._railArgs = rail; ev._hudText = toast; ev._kvDone = true;
  return true;
}
