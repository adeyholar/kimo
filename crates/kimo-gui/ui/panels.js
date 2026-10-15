// Panel renderers. Each kind is a pure function of (spec, data, w, h) that
// returns markup; geometry comes from the measured panel box and fixed row
// heights, so a value changing never changes a box.
const E = s => String(s ?? '').replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const px = n => Math.round(n * 10) / 10;

export function fmtArgs(a) {
  if (a == null) return '';
  if (typeof a === 'string') return a;
  if (Array.isArray(a)) return a.map(x => (x && typeof x === 'object') ? Object.entries(x).map(([k, v]) => `${k}: ${v}`).join(', ') : String(x)).join(', ');
  return Object.entries(a).map(([k, v]) => `${k}: ${v}`).join(', ');
}
const fmtDur = ms => (ms == null ? '' : (ms / 1000).toFixed(1) + 's');

function rng(seed) { let s = (seed * 9301 + 49297) % 233280 || 1; return () => (s = (s * 9301 + 49297) % 233280) / 233280; }
function head(spec, data, extra = '') {
  if (spec.title === undefined && !data.title) return '';
  const t = data.title ?? spec.title;
  const m = data.meta ?? spec.meta ?? '';
  return `<div class="ph"${spec.phPad ? ` style="padding:${spec.phPad}"` : ''}><span class="t">${E(t)}${data.titleSmall ? `<small>${E(data.titleSmall)}</small>` : ''}</span><span class="m">${E(m)}${extra}</span></div>` +
    (data.sub ? `<div class="psub">${data.subHtml ? data.sub : E(data.sub)}</div>` : '');
}

// ---------- agent ----------
export function tickRunning(state) {
  for (const el of document.querySelectorAll('[data-run]')) {
    const c = state.calls.find(x => String(x.id) === el.dataset.run);
    if (c && c.status === 'running') el.textContent = fmtDur(performance.now() - c.t);
  }
}
function agent({ spec, state, data, w, h }) {
  const o = spec.opts || {};
  const rowH = o.rowH || 37, top = o.top ?? 42, gap = o.gap || 0;
  const reserve = (o.reserve || 0);
  const slots = Math.max(1, Math.floor((h - top - reserve + gap) / (rowH + gap)));
  const calls = state.calls.slice(-slots);
  const meta = o.count ? `${state.callCount} calls` : '';
  let html = head(spec, { ...data, meta: data.meta ?? meta });
  html += `<div class="calls ${o.chips ? 'chips' : ''}">`;
  calls.forEach((c, i) => {
    const cur = i === calls.length - 1;
    const y = top + i * (rowH + gap);
    const ck = c.status === 'running' ? (o.runMark ?? '') : (c.status === 'error' ? '✕' : '✓');
    const showCk = !(cur && o.noCheckCur);
    const ckEl = `<span class="ck">${showCk ? ck : ''}</span>`;
    const dur = c.status === 'running' ? fmtDur(state.session.still && c.elapsed_ms != null ? c.elapsed_ms : performance.now() - c.t) : fmtDur(c.dur_ms);
    const args = fmtArgs(c.args);
    const faded = !cur && o.fade != null && i < calls.length - 1 - o.fade;
    const ch = cur && o.curH ? o.curH : rowH;
    const recent = o.boldLast && i >= calls.length - o.boldLast;
    html += `<div class="call ${cur ? 'cur' : 'old'}${faded ? ' faded' : ''}${recent ? ' recent' : ''}" style="top:${y}px;height:${ch}px${ch !== rowH ? `;align-items:flex-start;padding-top:${o.curPadT ?? 9}px` : ''}">` +
      (o.checkRight ? '' : ckEl) +
      `<span class="sig"><span class="fn">${E(c.name)}</span><span class="pp">(</span><span class="args">${E(args)}</span><span class="pp">)</span></span>` +
      (o.checkRight ? ckEl : '') +
      `<span class="dur" ${c.status === 'running' ? `data-run="${E(c.id)}"` : ''}>${dur}</span></div>`;
    if (cur && c.progress && o.progress) {
      const p = c.progress;
      html += `<div class="callbar" style="top:${y + rowH + 6}px;left:${o.barL ?? 15}px;right:${o.barR ?? 60}px"><i style="width:${(100 * p.done / p.total).toFixed(1)}%"></i></div>` +
        `<div class="mono dim" style="position:absolute;right:14px;top:${y + rowH - 1}px;font-size:${o.progFs ?? 11.5}px">${p.done} / ${p.total}</div>`;
    }
  });
  html += '</div>';
  if (data.footer) html += `<div class="pfoot">${E(data.footer)}</div>`;
  return html;
}

// ---------- viewer ----------
function viewer({ spec, data, w, h }) {
  const o = spec.opts || {};
  const [pt, pr, pb, pl] = o.pad || [12, 12, 44, 12];
  const fw = w - pl - pr, fh = h - pt - pb;
  let html = '';
  if (spec.title) html += head(spec, data);
  html += `<div class="frame" style="left:${pl}px;top:${pt}px;width:${fw}px;height:${fh}px">` +
    (data.src ? `<img src="${E(data.src)}" alt="">` : '<div class="ph-frame"></div>') +
    // a running grade/effect wipes across the frame: before (ungraded) on the right of the edge, result on the left
    (data.src && data.wipe != null && data.wipe < 1 ? `<img class="before" src="${E(data.src)}" alt="" style="clip-path:inset(0 0 0 ${px(data.wipe * 100)}%)"><i class="wipe" style="left:${px(data.wipe * fw)}px"></i>` : '');
  if (data.overlay) html += `<svg class="ov" viewBox="0 0 ${fw} ${fh}">${overlay(data.overlay, fw, fh)}</svg>`;
  for (const t of (data.tags || (data.tag ? [{ text: data.tag }] : []))) {
    const pos = t.pos === 'tr' ? `right:${t.x ?? 10}px` : `left:${t.x ?? 10}px`;
    html += `<span class="tag" style="${pos};top:${t.y ?? 10}px">${E(t.text)}</span>`;
  }
  html += '</div>';
  if (data.progress) {
    const p = data.progress, y = pt + fh + (o.progY ?? 14);
    html += `<div style="position:absolute;left:${pl}px;right:${pr}px;top:${y}px;height:3px;background:var(--raise2);border-radius:2px"><div style="width:${p.pct}%;height:100%;background:var(--ember);border-radius:2px"></div></div>` +
      `<div class="vmeta" style="top:${y + 9}px;left:${pl}px;right:${pr}px"><span>${E(p.left)}</span><span class="r"><span class="em">${E(p.pct)}%</span>&nbsp;&nbsp;&nbsp;${E(p.right)}</span></div>`;
  }
  if (data.thumbs) {
    const ty = o.thumbY ?? (pt + fh + 12), th = o.thumbH ?? 50;
    html += `<div class="thumbs" style="left:${pl}px;right:${pr}px;top:${ty}px;height:${th}px;gap:${o.thumbGap ?? 6}px;overflow:hidden">` +
      data.thumbs.map(t => `<div class="thumb ${t.sel ? 'sel' : ''} ${t.dim ? 'dimmed' : ''}"${o.thumbW ? ` style="flex:none;width:${o.thumbW}px"` : ''}>${t.src ? `<img src="${E(t.src)}" alt="">` : ''}${t.check ? '<span class="badge">✓</span>' : ''}</div>`).join('') + '</div>';
  }
  const my = o.metaY ?? (pt + fh + (pb - 20) / 2);
  if (data.tc || data.left || data.res) {
    html += `<div class="vmeta" style="top:${my}px;left:${o.metaL ?? 14}px;right:${o.metaR ?? 14}px">` +
      `<span>${E(data.left || '')}</span>` +
      (data.tc ? `<span class="tc" style="${o.tcLeft ? 'position:static;transform:none;font-weight:400;color:var(--dim);font-size:var(--fs-s)' : ''}">${E(data.tc)}</span>` : '') +
      `<span class="r">${E(data.res || '')}</span></div>`;
  }
  return html;
}

function overlay(ov, w, h) {
  let s = '';
  for (const o of ov) {
    if (o.quad) {
      const p = o.quad.map(([x, y]) => [x * w, y * h]);
      s += `<polygon points="${p.map(q => q.join(',')).join(' ')}" fill="rgba(255,90,35,.08)" stroke="var(--ember)" stroke-width="1.5"/>`;
      p.forEach(q => s += `<circle cx="${q[0]}" cy="${q[1]}" r="4" fill="var(--ember)"/>`);
    }
    if (o.vline != null) s += `<line x1="${o.vline * w}" x2="${o.vline * w}" y1="0" y2="${h}" stroke="var(--ember)" stroke-width="2"/>`;
  }
  return s;
}

// ---------- key/value inspector ----------
function kv({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  html += `<div class="kv ${o.lines ? 'lines' : ''} ${o.topLine ? 'topline' : ''}" style="top:${o.top ?? (data.sub ? 69 : 46)}px${o.pad != null ? `;padding:0 ${o.pad}px` : ''}">`;
  for (const b of (data.blocks || [])) html += block(b, w - 30);
  return html + '</div>';
}
function block(b, w) {
  if (b.sec !== undefined) return `<div class="sec" ${b.h ? `style="height:${b.h}px"` : ''}>${E(b.sec)}</div>`;
  if (b.gap) return `<div style="height:${b.gap}px"></div>`;
  if (b.list) return `<div class="list">${b.list.map((x, i) => `<div class="${i === b.sel ? 'sel' : ''}">${E(x)}</div>`).join('')}</div>`;
  if (b.code) return `<div class="code">${E(b.code)}</div>`;
  if (b.cap) return `<div class="cap"><span>${E(b.cap[0])}</span><span>${E(b.cap[1] || '')}</span></div>`;
  if (b.widget) return `<div class="widget" style="height:${b.h || 90}px${b.style ? ';' + b.style : ''}">${(WIDGETS[b.widget] || (() => ''))(b, w, b.h || 90)}</div>`;
  if (b.html) return b.html;
  const cls = ['row', b.from != null || b.ch ? 'ch' : '', b.chip ? 'chip' : '', b.emb ? 'emb' : '', b.bar ? 'bar' : ''].join(' ');
  const v = b.from != null ? `<span class="from">${E(b.from)}</span><span class="arr">→</span>${E(b.v)}` : E(b.v);
  return `<div class="${cls}"${b.h ? ` style="height:${b.h}px"` : ''}><span class="k"${b.kStyle ? ` style="${b.kStyle}"` : ''}>${E(b.k)}</span><span class="v"${b.vStyle ? ` style="${b.vStyle}"` : ''}>${v}</span></div>`;
}

// ---------- small inspector widgets (SVG from data) ----------
const WIDGETS = {
  eq(b, w, h) { // b.bands: [{f,g,q,type}] ; draws response curve on log axis 20..20k
    const fx = f => (Math.log10(f) - Math.log10(20)) / 3 * w, gy = g => h / 2 - g * (h / 2 - 10) / 18;
    let grid = '';
    for (const f of [100, 1000, 10000]) grid += `<line x1="${fx(f)}" x2="${fx(f)}" y1="0" y2="${h}" stroke="#1d1d1c"/>`;
    for (const g of [-12, 0, 12]) grid += `<line x1="0" x2="${w}" y1="${gy(g)}" y2="${gy(g)}" stroke="#1d1d1c"/>`;
    const resp = f => (b.bands || []).reduce((s, B) => {
      const x = Math.log2(f / B.f);
      if (B.type === 'lowshelf') return s + B.g / (1 + Math.exp(x * 4));
      if (B.type === 'highshelf') return s + B.g / (1 + Math.exp(-x * 4));
      return s + B.g * Math.exp(-(x * x) * (B.q || 1) * 1.6);
    }, 0);
    const pts = []; for (let i = 0; i <= 120; i++) { const f = 20 * Math.pow(1000, i / 120); pts.push([fx(f), gy(resp(f))]); }
    const d = pts.map((p, i) => (i ? 'L' : 'M') + px(p[0]) + ' ' + px(p[1])).join('');
    let s = grid + `<path d="${d} L${w} ${h / 2} L0 ${h / 2}Z" fill="rgba(243,240,231,.06)"/><path d="${d}" fill="none" stroke="var(--cream)" stroke-width="1.4"/>`;
    for (const B of (b.bands || [])) s += `<circle cx="${px(fx(B.f))}" cy="${px(gy(resp(B.f)))}" r="3.5" fill="${B.sel ? 'var(--ember)' : 'var(--cream)'}"/>`;
    s += [100, 1000, 10000].map((f, i) => `<text x="${fx(f) + 3}" y="${h - 5}" font-size="9" fill="var(--dim)">${['100', '1k', '10k'][i]}</text>`).join('');
    return `<svg viewBox="0 0 ${w} ${h}">${s}</svg>`;
  },
  comp(b, w, h) { // compressor transfer curve + text rows
    const s = Math.min(h - 12, 64), x0 = 8, y0 = (h - s) / 2, th = b.threshold ?? -18, r = b.ratio ?? 3;
    const map = db => (db + 60) / 60; const tx = map(th);
    const knee = [x0 + tx * s, y0 + s - tx * s];
    const end = [x0 + s, y0 + s - (tx + (1 - tx) / r) * s];
    let g = `<rect x="${x0}" y="${y0}" width="${s}" height="${s}" fill="none" stroke="#242423"/>` +
      `<line x1="${x0}" y1="${y0 + s}" x2="${x0 + s}" y2="${y0}" stroke="#242423" stroke-dasharray="2 3"/>` +
      `<path d="M${x0} ${y0 + s} L${knee[0]} ${knee[1]} L${end[0]} ${end[1]}" fill="none" stroke="var(--cream)" stroke-width="1.4"/>` +
      `<circle cx="${knee[0]}" cy="${knee[1]}" r="3" fill="var(--ember)"/>`;
    const tx2 = x0 + s + 16;
    g += `<text x="${tx2}" y="${h / 2 - 6}" font-size="11.5" fill="var(--dim)" style="font-family:var(--label-font)">Threshold</text><text x="${w - 8}" y="${h / 2 - 6}" font-size="11.5" fill="var(--cream)" text-anchor="end">${th} dB</text>` +
      `<text x="${tx2}" y="${h / 2 + 16}" font-size="11.5" fill="var(--dim)" style="font-family:var(--label-font)">Ratio</text><text x="${w - 8}" y="${h / 2 + 16}" font-size="11.5" fill="var(--cream)" text-anchor="end">${r}:1</text>`;
    return `<svg viewBox="0 0 ${w} ${h}">${g}</svg>`;
  },
  solve(b, w, h) { // camera solve: point cloud + frustum + dotted orbit
    const R = rng(b.seed || 5); let s = '';
    for (let i = 0; i < 260; i++) { const a = R() * Math.PI * 2, rr = Math.pow(R(), .6) * h * .32; s += `<circle cx="${px(w * .5 + Math.cos(a) * rr * 1.7)}" cy="${px(h * .55 + Math.sin(a) * rr * .55)}" r="${R() < .2 ? 1.2 : .7}" fill="${R() < .5 ? '#cfcac0' : '#77736b'}"/>`; }
    s += `<ellipse cx="${w * .5}" cy="${h * .55}" rx="${w * .38}" ry="${h * .22}" fill="none" stroke="#8a867d" stroke-dasharray="2 4"/>`;
    const cx = w * .22, cy = h * .32;
    s += `<path d="M${cx} ${cy} l22 -9 l0 22 z" fill="none" stroke="var(--ember)" stroke-width="1.3"/><circle cx="${cx}" cy="${cy}" r="2.5" fill="var(--ember)"/>`;
    return `<svg viewBox="0 0 ${w} ${h}">${s}</svg>`;
  },
  swatches(b, w, h) {
    const n = b.items.length, gap = 10, d = Math.min(h - 6, (w - gap * (n - 1)) / n);
    let s = '<defs>' + b.items.map((it, i) => `<radialGradient id="sw${i}" cx="35%" cy="30%" r="75%"><stop offset="0" stop-color="${it.hi}"/><stop offset=".55" stop-color="${it.mid}"/><stop offset="1" stop-color="${it.lo}"/></radialGradient>`).join('') + '</defs>';
    b.items.forEach((it, i) => {
      const x = i * (d + gap) + d / 2;
      s += `<rect x="${x - d / 2 + .75}" y="${.75}" width="${d - 1.5}" height="${d - 1.5}" rx="6" fill="#141413" stroke="${it.sel ? 'var(--cream)' : '#2a2a28'}" stroke-width="${it.sel ? 1.5 : 1}"/><circle cx="${x}" cy="${d / 2}" r="${d / 2 - 7}" fill="url(#sw${i})"/>`;
    });
    return `<svg viewBox="0 0 ${w} ${h}">${s}</svg>`;
  },
  thumbs(b, w, h) {
    const n = b.items.length, gap = b.gap ?? 8, tw = (w - gap * (n - 1)) / n;
    return `<div style="display:flex;gap:${gap}px;height:100%">` + b.items.map(t => `<div class="thumb ${t.sel ? 'sel' : ''}" style="flex:1;height:${t.label ? h - 18 : h}px">${t.src ? `<img src="${E(t.src)}">` : ''}</div>`).join('') + '</div>' +
      (b.items.some(t => t.label) ? `<div style="display:flex;gap:${gap}px;margin-top:3px">${b.items.map(t => `<span style="flex:1;text-align:center;font-family:var(--mono);font-size:10.5px;color:var(--dim)">${E(t.label || '')}</span>`).join('')}</div>` : '');
  },
};

// ---------- timeline (edit / effects / graphics / captions / audio) ----------
function rulerX(r, x0, x1) { return t => x0 + (t - r.t0) / (r.t1 - r.t0) * (x1 - x0); }
function fmtT(t, f) {
  if (f === 'n') return String(t);
  if (f === 's') return t + 's';
  const m = Math.floor(t / 60), s = Math.round(t % 60);
  return String(m).padStart(2, '0') + ':' + String(s).padStart(2, '0');
}
function ruler(r, X, y, o = {}) {
  let s = `<div class="ruler" style="top:${y}px">`;
  if (r.minor) for (let t = r.minorFrom ?? r.t0; t <= r.t1 + 1e-6; t += r.minor) s += `<i class="mn" style="left:${px(X(t))}px"></i>`;
  const last = r.last ?? r.t1;
  for (let t = r.first ?? r.t0; t <= last + 1e-6; t += r.step) {
    const x = X(t);
    const tr = r.edge && t === r.t0 ? 'none' : r.edge && t + r.step > last + 1e-6 ? 'translateX(-100%)' : r.align === 'right' ? 'translateX(-100%) translateX(-5px)' : r.align === 'left' ? `translateX(${r.lx ?? 4}px)` : 'translateX(-50%)';
    s += `<span style="left:${px(x)}px;transform:${tr}">${fmtT(Math.round(t * 100) / 100, r.fmt)}</span>`;
    if (r.ticks !== false) s += `<i style="left:${px(x)}px"></i>`;
  }
  return s + '</div>';
}
function wave(w, h, seed, amp = .8, dens = 1, sym = true) {
  const R = rng(seed); const n = Math.max(8, Math.floor(w / 2.2 * dens)); const up = [], dn = [];
  let env = .5;
  for (let i = 0; i <= n; i++) {
    env += (R() - .5) * .35; env = Math.max(.15, Math.min(1, env));
    const a = (R() * .55 + .45) * env * amp * (h / 2);
    const x = i / n * w; up.push(`${px(x)},${px(h / 2 - a)}`); dn.push(`${px(x)},${px(h / 2 + (sym ? a : a * .9))}`);
  }
  return `M${up.join(' L')} L${dn.reverse().join(' L')}Z`;
}
function smoothArea(w, h, a) {
  const R = rng(a.seed || 1); const pts = [];
  if (a.pts) a.pts.forEach((v, i) => pts.push([i / (a.pts.length - 1) * w, v * h]));  // explicit envelope (0 = top)
  else { const n = 9; for (let i = 0; i <= n; i++) pts.push([i / n * w, h * (a.base ?? .5) + (R() - .5) * h * (a.amp ?? .35)]); }
  let d = `M0 ${h} L${px(pts[0][0])} ${px(pts[0][1])}`;
  for (let i = 1; i < pts.length; i++) { const [x0, y0] = pts[i - 1], [x1, y1] = pts[i], mx = (x0 + x1) / 2; d += ` C${px(mx)} ${px(y0)} ${px(mx)} ${px(y1)} ${px(x1)} ${px(y1)}`; }
  return d + ` L${px(w)} ${h}Z`;
}
function timeline({ spec, data, w, h }) {
  const o = spec.opts || {};
  const x0 = o.x0 ?? 48, x1 = w - (o.xr ?? 12);
  const r = data.ruler || { t0: 0, t1: 20, step: 5 };
  const X = rulerX(r, x0, x1);
  let html = head(spec, data);
  html += `<div class="tl">` + ruler(r, X, o.rulerY ?? 6);
  let y = o.trackY ?? 30;
  for (const t of (data.tracks || [])) {
    const th = t.h ?? o.trackH ?? 38;
    html += `<div class="trk ${t.cls || ''}" style="top:${y}px;height:${th}px">`;
    if (t.area) html += `<svg style="position:absolute;left:${px(X(t.area.s ?? r.t0))}px;top:0;width:${px((x1) - X(t.area.s ?? r.t0))}px;height:${th}px" viewBox="0 0 ${px(x1 - X(t.area.s ?? r.t0))} ${th}"><rect width="100%" height="100%" rx="7" fill="${t.area.bg || '#2a2824'}"/><path d="${smoothArea(x1 - X(t.area.s ?? r.t0), th, t.area)}" fill="${t.area.fill || '#4a4741'}"/></svg>`;
    if (t.label) html += t.chip ? `<span class="labchip" style="left:${o.chipX ?? 10}px;width:${o.chipW ?? 120}px">${E(t.label)}</span>` : `<span class="lab">${E(t.label)}</span>`;
    if (t.lane) { const li = t.laneInset ?? o.laneInset ?? 3; html += `<div class="lane" style="position:absolute;left:${x0}px;right:${w - x1}px;top:${li}px;bottom:${li}px;background:${t.lane}"></div>`; }
    if (t.wave) {
      const a = X(t.wave.s ?? r.t0), b = X(t.wave.e ?? r.t1), wh = th - 6;
      html += `<svg style="position:absolute;left:${px(a)}px;top:3px;width:${px(b - a)}px;height:${wh}px" viewBox="0 0 ${px(b - a)} ${wh}"><path d="${wave(b - a, wh, t.wave.seed || 3, t.wave.amp ?? .8, t.wave.dens ?? 1)}" fill="${t.wave.color || '#5f5c56'}"/>` +
        (t.wave.env ? `<path d="${t.wave.env.map((p, i) => (i ? 'L' : 'M') + px(X(p[0]) - a) + ' ' + px(wh * (1 - p[1]))).join('')}" fill="none" stroke="var(--ember)" stroke-width="1.5"/>` : '') +
        (t.wave.beats ? t.wave.beats.map(bt => `<line x1="${px(X(bt) - a)}" x2="${px(X(bt) - a)}" y1="0" y2="5" stroke="var(--cream)" stroke-width="1.2"/>`).join('') : '') +
        `</svg>`;
    }
    for (const c of (t.clips || [])) {
      const a = X(c.s), b = X(c.e);
      html += `<div class="clip ${c.cls || ''}" style="left:${px(a)}px;width:${px(b - a)}px${c.bg ? ';background:' + c.bg : ''}">` +
        (c.thumb ? `<span class="ct"><img src="${E(c.thumb)}" alt=""></span>` : '') + (c.cover ? `<img src="${E(c.cover)}" alt="">` : '') + (c.label ? `<span class="cl">${E(c.label)}</span>` : '') +
        (c.pill ? `<span class="cpill">${E(c.pill)}</span>` : '') +
        (c.fx ? (o.fxRow ? `<span class="fxrow">${c.fx.map(f => `<span class="fxchip">${E(f)}</span>`).join('')}</span>` : c.fx.map(f => `<span class="fxchip">${E(f)}</span>`).join('')) : '') + '</div>';
    }
    html += '</div>';
    y += th + (o.trackGap ?? 0);
  }
  if (data.playhead != null) html += `<div class="playhead" style="left:${px(X(data.playhead))}px;top:${o.phTop ?? 4}px;bottom:${o.phBot ?? 8}px"></div>`;
  return html + '</div>';
}

// ---------- key lanes (mask keys, keyframes, simulation cache, residual) ----------
function keys({ spec, data, w, h }) {
  const o = spec.opts || {};
  const x0 = o.x0 ?? 190, x1 = w - (o.xr ?? 14);
  const r = data.ruler; const X = rulerX(r, x0, x1);
  let html = head(spec, data, data.metaChip ? `<span class="chip" style="padding:3px 8px;border-radius:4px;background:var(--raise);color:var(--text)">${E(data.metaChip)}</span>` : '');
  html += '<div class="tl">';
  if (data.strip) { // mini curve strip above ruler
    const sy = o.stripY ?? 4, sh = o.stripH ?? 40;
    let s = '';
    for (const c of data.strip) s += `<path d="${c.pts.map((p, i) => (i ? 'L' : 'M') + px(X(p[0])) + ' ' + px(sh - 4 - p[1] * (sh - 8))).join('')}" fill="none" stroke="${c.em ? 'var(--ember)' : '#bdb8ae'}" stroke-width="1.4"/>`;
    html += `<svg style="position:absolute;left:0;top:${sy}px;width:${w}px;height:${sh}px" viewBox="0 0 ${w} ${sh}">${s}</svg>`;
  }
  if (data.graph) { // residual-style graph box
    const g = data.graph, gy = o.graphY ?? 30, gh = o.graphH ?? 76, gx = o.graphX ?? x0;
    const vy = v => gh - 6 - (v / g.max) * (gh - 14);
    const d = g.pts.map((v, i) => (i ? 'L' : 'M') + px(X(r.t0 + i * (r.t1 - r.t0) / (g.pts.length - 1)) - gx) + ' ' + px(vy(v))).join('');
    html += `<svg style="position:absolute;left:${gx}px;top:${gy}px;width:${x1 - gx + 8}px;height:${gh}px;background:#121211;border-radius:4px" viewBox="0 0 ${x1 - gx + 8} ${gh}">` +
      `<line x1="0" x2="${x1 - gx + 8}" y1="${vy(g.thr)}" y2="${vy(g.thr)}" stroke="#8a867d" stroke-dasharray="3 3"/><text x="${x1 - gx}" y="${vy(g.thr) - 4}" font-size="10" fill="var(--dim)" text-anchor="end">${E(g.thrLabel)}</text>` +
      `<path d="${d}" fill="none" stroke="var(--cream)" stroke-width="1.3"/></svg>`;
  }
  html += ruler(r, X, o.rulerY ?? 6);
  let y = o.laneY ?? 32;
  for (const L of data.lanes || []) {
    const lh = L.h ?? o.laneH ?? 32;
    html += `<div class="trk" style="top:${y}px;height:${lh}px${L.sel ? ';background:rgba(255,90,35,.07)' : ''}">`;
    html += `<span class="labchip" style="left:${o.chipX ?? 10}px;width:${o.chipW ?? (x0 - 20)}px">${L.html ? L.label : E(L.label)}</span>`;
    const mid = lh / 2;
    let s = '';
    if (L.bar) for (const [a, b, c] of L.bar) s += `<rect x="${px(X(a))}" y="${px(mid - (o.barH ?? 9))}" width="${px(X(b) - X(a))}" height="${(o.barH ?? 9) * 2}" rx="3" fill="${c || '#6b665d'}"/>`;
    if (L.cache) { const [done, total] = L.cache; s += `<rect x="${x0}" y="${mid - 9}" width="${px(X(done) - x0)}" height="18" rx="3" fill="#bdb8ae"/><rect x="${px(X(done))}" y="${mid - 9}" width="${px(x1 - X(done))}" height="18" rx="3" fill="#070707" stroke="#2a2a28"/>`; for (let f = done + 2; f < total; f += 2) s += `<line x1="${px(X(f))}" x2="${px(X(f))}" y1="${mid - 4}" y2="${mid + 4}" stroke="#2a2a28"/>`; }
    if (L.area) { const pts = L.area; s += `<path d="M${px(X(pts[0][0]))} ${mid + 9} ${pts.map(p => `L${px(X(p[0]))} ${px(mid + 9 - p[1] * 18)}`).join(' ')} L${px(X(pts[pts.length - 1][0]))} ${mid + 9}Z" fill="${L.color || '#5a5650'}"/>`; }
    if (L.dots) for (const t of L.dots) s += `<circle cx="${px(X(t))}" cy="${mid}" r="${L.r || 2.6}" fill="${L.dotColor || '#77736b'}"/>`;
    if (L.keys) for (const k of L.keys) { const t = typeof k === 'number' ? k : k.t; const em = typeof k === 'object' && k.em; const d = L.size || 5.5; s += `<path d="M${px(X(t))} ${mid - d} L${px(X(t) + d)} ${mid} L${px(X(t))} ${mid + d} L${px(X(t) - d)} ${mid}Z" fill="${em ? 'var(--ember)' : (L.keyColor || 'var(--cream)')}"/>`; }
    html += `<svg style="position:absolute;left:0;top:0;width:${w}px;height:${lh}px" viewBox="0 0 ${w} ${lh}">${s}</svg></div>`;
    y += lh + (o.laneGap ?? 0);
  }
  if (data.playhead != null) html += `<div class="playhead" style="left:${px(X(data.playhead))}px;top:${o.phTop ?? 4}px;bottom:${o.phBot ?? 8}px"></div>`;
  return html + '</div>';
}

// ---------- graph editor (curves) ----------
function catmull(P) { // smooth path through points (uniform Catmull-Rom as cubic beziers)
  let d = `M${px(P[0][0])} ${px(P[0][1])}`;
  for (let i = 0; i < P.length - 1; i++) {
    const p0 = P[i - 1] || P[i], p1 = P[i], p2 = P[i + 1], p3 = P[i + 2] || p2;
    const c1 = [p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6], c2 = [p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6];
    d += ` C${px(c1[0])} ${px(c1[1])} ${px(c2[0])} ${px(c2[1])} ${px(p2[0])} ${px(p2[1])}`;
  }
  return d;
}
function curves({ spec, data, w, h }) {
  const o = spec.opts || {};
  const x0 = o.x0 ?? 196, x1 = w - (o.xr ?? 12);
  const r = data.ruler; const X = rulerX(r, x0, x1);
  const gy0 = o.gy0 ?? 72, gy1 = h - (o.gyb ?? 10);
  let html = head(spec, data) + '<div class="tl">';
  let s = '';
  const gl = o.gridC || '#1a1a19';
  if (o.band != null) s += `<rect x="${o.bandX ?? x0}" y="${o.band[0]}" width="${w - (o.bandX ?? x0)}" height="${o.band[1] - o.band[0]}" fill="${o.bandC || '#171715'}"/>`;
  const hl = o.hlines || 4;
  for (let i = 0; i <= hl; i++) { const y = gy0 + i * (gy1 - gy0) / hl; s += `<line x1="${o.bandX ?? x0}" x2="${w}" y1="${px(y)}" y2="${px(y)}" stroke="${gl}"/>`; }
  for (let t = r.first ?? r.t0; t <= r.t1; t += r.step) s += `<line x1="${px(X(t))}" x2="${px(X(t))}" y1="${o.vTop ?? gy0 - 6}" y2="${gy1}" stroke="${gl}"/>`;
  const bs = o.beatS ?? 4;
  if (data.beats) for (let t = data.beats.t0; t <= r.t1; t += data.beats.step) { const x = X(t), y = o.beatY ?? 14; s += `<path d="M${px(x)} ${y - bs} L${px(x + bs)} ${y} L${px(x)} ${y + bs} L${px(x - bs)} ${y}Z" fill="#d9d4c9"/>`; }
  const V = v => gy1 - v * (gy1 - gy0);
  const ks = o.keyS ?? 7;
  for (const c of data.curves || []) {
    const P = c.pts.map(p => [X(p[0]), V(p[1])]);
    const d = c.smooth ? catmull(P) : P.map((q, i) => (i ? 'L' : 'M') + px(q[0]) + ' ' + px(q[1])).join('');
    const col = c.em ? 'var(--ember)' : c.color || 'var(--cream)';
    s += `<path d="${d}" fill="none" stroke="${col}" stroke-width="${c.em ? (o.emW ?? 1.8) : (c.w ?? o.lineW ?? 1.4)}"/>`;
    for (const hd of (c.handles || [])) { const [a, b] = hd; s += `<line x1="${px(X(a[0]))}" y1="${px(V(a[1]))}" x2="${px(X(b[0]))}" y2="${px(V(b[1]))}" stroke="var(--ember)" stroke-width="1"/>` + [a, b].map(q => `<circle cx="${px(X(q[0]))}" cy="${px(V(q[1]))}" r="${o.hR ?? 3}" fill="${o.hFill ? 'var(--ember)' : 'var(--ink)'}" stroke="var(--ember)"/>`).join(''); }
    for (const k of (c.keys || [])) s += `<rect x="${px(X(k[0]) - ks / 2)}" y="${px(V(k[1]) - ks / 2)}" width="${ks}" height="${ks}" fill="${k[2] || col}" stroke="${o.keyStroke || 'none'}"/>`;
  }
  html += `<svg style="position:absolute;left:0;top:0;width:${w}px;height:${h}px" viewBox="0 0 ${w} ${h}">${s}</svg>`;
  html += ruler(r, X, o.rulerY ?? 34);
  (data.lanes || []).forEach((L, i) => {
    html += `<span class="labchip" style="position:absolute;transform:none;left:${o.chipX ?? 10}px;width:${o.chipW ?? 176}px;top:${(o.laneY ?? 76) + i * (o.laneStep ?? 34)}px">${E(L)}</span>`;
  });
  if (data.playhead != null) html += `<div class="playhead ${o.phCls ?? 'both'}" style="left:${px(X(data.playhead))}px;top:${o.phTop ?? 26}px;bottom:${o.phBot ?? 6}px"></div>`;
  return html + '</div>';
}

// ---------- transcript ----------
function transcript({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  (data.lines || []).forEach((L, i) => {
    html += `<div class="tline" style="top:${px((o.y0 ?? 47) + i * (o.step ?? 40))}px">` +
      `<span class="spk">${E(L.spk)}</span><span class="ttc">${E(L.tc)}</span><span class="tx">${L.html || E(L.text)}</span></div>`;
  });
  return html;
}

// ---------- history / checkpoints ----------
function history({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  const y0 = o.y0 ?? 48, step = o.step ?? 70;
  html += `<div style="position:absolute;left:${(o.tw ?? 70) / 2 + 15}px;top:${y0 + (o.th ?? 44) / 2}px;height:${step * ((data.items || []).length - 1)}px;width:var(--hist-line-w,1px);background:var(--hist-line,var(--line))"></div>`;
  (data.items || []).forEach((it, i) => {
    const y = y0 + i * step;
    html += `<div style="position:absolute;left:15px;right:15px;top:${y}px;height:${o.th ?? 44}px;display:flex;align-items:center;gap:12px">` +
      `<div class="thumb ${it.sel ? 'sel' : ''}" style="flex:none;width:${o.tw ?? 70}px;height:100%">${it.src ? `<img src="${E(it.src)}">` : ''}</div>` +
      `<div style="font-family:var(--mono);font-size:var(--hist-fs,12px);line-height:var(--hist-lh,17px)"><div class="cream">${E(it.id)}</div><div class="${o.nameCream ? 'cream' : 'dim'}">${E(it.name)}</div></div>` +
      `<span style="margin-left:auto;font-family:var(--mono);font-size:var(--hist-ago-fs,11.5px);color:var(--ago-c,var(--dim))">${E(it.ago)}${it.live ? ' <span class="em">●</span>' : ''}</span></div>`;
  });
  return html;
}

// ---------- diff + batch table ----------
function diffbatch({ spec, data, w, h }) {
  const o = spec.opts || {};
  const split = o.split ?? Math.round(w * .56);
  let html = head(spec, data);
  const col = o.col || { '+': '#cfe0b8', '~': 'var(--ember-text)', '-': '#e28a7a' };
  (data.diff || []).forEach((L, i) => {
    html += `<div style="position:absolute;left:15px;width:${split - 30}px;top:${(o.y0 ?? 52) + i * (o.step ?? 24)}px;display:flex;gap:var(--diff-gap,12px);font-family:var(--mono);font-size:var(--diff-fs,12px);white-space:pre"><span style="color:${col[L[0]]};width:10px">${E(L[0])}</span><span style="color:${L[0] === '~' && !o.col ? 'var(--text)' : (o.txt || col)[L[0]]}">${E(L[1])}</span></div>`;
  });
  html += `<div style="position:absolute;left:${split}px;top:${o.splitTop ?? 14}px;bottom:14px;width:1px;background:var(--line)"></div>`;
  const b = data.batch || {};
  html += `<div style="position:absolute;left:${split + (o.bx ?? 16)}px;right:${o.br ?? 15}px;top:${o.bty ?? 12}px;display:flex;justify-content:space-between;align-items:baseline;font-family:var(--title-font,var(--sans));font-weight:var(--batch-t-w,600);font-size:var(--batch-t-fs,13px)"><span>${E(b.title || '')}</span><span class="mono dim" style="font-weight:400;font-size:var(--batch-m-fs,inherit)${o.metaInline ? ';margin-left:10px;margin-right:auto;color:var(--cream);font-family:var(--sans)' : ''}">${E(b.meta || '')}</span></div>`;
  (b.rows || []).forEach((r, i) => {
    const st = r[2];
    html += `<div style="position:absolute;left:${split + (o.bx ?? 16)}px;right:${o.br ?? 15}px;top:${(o.by0 ?? 46) + i * (o.bstep ?? 24)}px;display:grid;grid-template-columns:var(--batch-cols,1fr 1fr 70px);font-family:var(--mono);font-size:var(--diff-fs,12px);${st === 'queued' ? 'opacity:.45' : ''}"><span style="color:${st === 'grading' ? 'var(--cream)' : (o.nameC || 'inherit')}">${E(r[0])}</span><span style="color:${o.lookC || 'var(--dim)'}">${E(r[1])}</span><span style="text-align:right;color:${st === 'grading' ? (o.activeC || 'var(--ember-text)') : (o.doneC || 'var(--dim)')}">${E(st)}</span></div>`;
  });
  return html;
}

// ---------- media list ----------
function media({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  (data.items || []).forEach((it, i) => {
    html += `<div style="position:absolute;left:15px;right:15px;top:${(o.y0 ?? 46) + i * (o.step ?? 32)}px;height:${o.th ?? 26}px;display:flex;align-items:center;gap:${o.gap ?? 10}px;font-family:var(--mono);font-size:var(--media-fs,12px)">` +
      `<div class="thumb" style="flex:none;width:${o.tw ?? 38}px;height:${o.thh ?? 24}px">${it.src ? `<img src="${E(it.src)}">` : (it.wave ? `<svg viewBox="0 0 38 24" style="width:38px;height:24px"><path d="${wave(38, 24, 9, .9)}" fill="#8a867d"/></svg>` : '')}</div>` +
      `<span class="cream">${E(it.name)}</span><span style="margin-left:auto;display:flex;align-items:center;gap:6px;font-size:${o.noteFs ? o.noteFs + 'px' : 'inherit'};${o.noteC ? 'color:' + o.noteC : ''}" class="${it.em ? 'em' : (o.noteC ? '' : 'dim')}">${it.link ? '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M6.5 9.5l3-3M7 4.5l1-1a3 3 0 0 1 4.2 4.2l-1 1M9 11.5l-1 1a3 3 0 0 1-4.2-4.2l1-1"/></svg>' : ''}${E(it.note || '')}</span></div>`;
  });
  if (data.foot) html += `<div class="pfoot">${E(data.foot)}</div>`;
  return html;
}

// ---------- scopes ----------
function lumaTrace(w, h, seed, color, alpha, shape) {
  // dense vertical scatter: for each column, a distribution of luma values
  const R = rng(seed); let s = '';
  const cols = Math.floor(w / 1.6);
  for (let i = 0; i < cols; i++) {
    const x = i / cols * w, u = i / cols;
    const base = shape ? shape(u) : .45 + .25 * Math.sin(u * 6.3 + 1);
    for (let k = 0; k < 9; k++) {
      const v = Math.max(.02, Math.min(.98, base + (R() - .5) * .38 * (k < 3 ? .4 : 1)));
      s += `<rect x="${px(x)}" y="${px(h - v * h)}" width="1.2" height="${px(1 + R() * 3)}" fill="${color}" opacity="${alpha}"/>`;
    }
  }
  return s;
}
function scopeAxis(x, y0, y1) {
  return [1023, 768, 512, 256, 0].map((v, i) => `<text x="${x}" y="${px(y0 + i * (y1 - y0) / 4 + 3)}" font-size="9.5" fill="var(--dim)" text-anchor="end">${v}</text><line x1="${x + 6}" x2="9999" y1="${px(y0 + i * (y1 - y0) / 4)}" y2="${px(y0 + i * (y1 - y0) / 4)}" stroke="#1a1a19"/>`).join('');
}
function vectorscope(cx, cy, r, seed) {
  let s = `<circle cx="${cx}" cy="${cy}" r="${r}" fill="none" stroke="#e0613a" stroke-width="1.6"/><circle cx="${cx}" cy="${cy}" r="${r * .66}" fill="none" stroke="#2a2a28"/><circle cx="${cx}" cy="${cy}" r="${r * .33}" fill="none" stroke="#2a2a28"/>` +
    `<line x1="${cx - r}" x2="${cx + r}" y1="${cy}" y2="${cy}" stroke="#2a2a28"/><line x1="${cx}" x2="${cx}" y1="${cy - r}" y2="${cy + r}" stroke="#2a2a28"/>`;
  const targets = [[103, 'R'], [167, 'Mg'], [241, 'B'], [283, 'Cy'], [347, 'G'], [61, 'Yl']];
  for (const [a] of targets) { const t = a * Math.PI / 180; s += `<rect x="${px(cx + Math.cos(t) * r * .75 - 4)}" y="${px(cy - Math.sin(t) * r * .75 - 4)}" width="8" height="8" fill="none" stroke="#5a5650"/>`; }
  const sk = 123 * Math.PI / 180; s += `<line x1="${cx}" y1="${cy}" x2="${px(cx + Math.cos(sk) * r)}" y2="${px(cy - Math.sin(sk) * r)}" stroke="#bdb8ae" stroke-width="1"/>`;
  const R = rng(seed);
  for (let i = 0; i < 700; i++) {
    const teal = R() < .45; const a = (teal ? 300 : 120) * Math.PI / 180 + (R() - .5) * .7; const rr = Math.pow(R(), 1.8) * r * (teal ? .5 : .62);
    s += `<circle cx="${px(cx + Math.cos(a) * rr)}" cy="${px(cy - Math.sin(a) * rr)}" r=".9" fill="${teal ? '#4fb3b0' : '#f08a4b'}" opacity=".75"/>`;
  }
  return s;
}
function scopes({ spec, data, w, h }) {
  const o = spec.opts || {};
  if (!data.wave) {   // legacy synthetic look (no binned scope data from the engine)
    const split = o.split ?? Math.round(w * .5);
    let s = '';
    const y0 = o.y0 ?? 18, y1 = h - (o.yb ?? 34);
    s += `<g>${scopeAxis(44, y0, y1)}</g>`;
    s += `<svg x="52" y="${y0}" width="${split - 66}" height="${y1 - y0}">${lumaTrace(split - 66, y1 - y0, 4, '#e9e4d8', .5)}</svg>`;
    s += `<line x1="${split}" x2="${split}" y1="12" y2="${h - 12}" stroke="var(--line)"/>`;
    const cx = split + (w - split) / 2, cy = (y0 + y1) / 2, r = Math.min((w - split) / 2 - 20, (y1 - y0) / 2 + 6);
    s += vectorscope(cx, cy, r, 7);
    return head(spec, data) + `<svg style="position:absolute;inset:0" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${s}</svg>`;
  }
  // Binned scope data (what an engine's render_scope sends): drawn on canvas after mount.
  const g = scopeGeom(o, w, h);
  let s = '';
  [1023, 768, 512, 256, 0].forEach(v => {
    const y = g.wy1 - v / 1023 * (g.wy1 - g.wy0);
    s += `<line x1="${g.wx0}" x2="${g.wx1}" y1="${px(y)}" y2="${px(y)}" stroke="#1f1f1d"/><text x="${g.wx0 - 8}" y="${px(y + 4.5)}" text-anchor="end" font-size="${o.axFs ?? 13}" fill="var(--dim)" font-family="var(--mono)">${v}</text>`;
  });
  for (let x = g.wx0; x <= g.wx1; x += (o.vgrid ?? 21.2)) s += `<line x1="${px(x)}" x2="${px(x)}" y1="${g.wy0}" y2="${g.wy1}" stroke="#1a1a18"/>`;
  if (o.splitLine) s += `<line x1="${g.split}" x2="${g.split}" y1="0" y2="${h}" stroke="var(--line)"/>`;
  s += `<text x="${px((g.wx0 + g.wx1) / 2)}" y="${h - (o.labB ?? 5)}" text-anchor="middle" font-size="${o.labFs ?? 15}" fill="var(--dim)" font-family="var(--mono)">waveform</text>`;
  s += `<text x="${g.vx}" y="${h - (o.labB ?? 5)}" text-anchor="middle" font-size="${o.labFs ?? 15}" fill="var(--dim)" font-family="var(--mono)">vectorscope</text>`;
  return head(spec, data) + `<canvas class="scopecv" width="${w}" height="${h}" style="position:absolute;left:0;top:0;width:${w}px;height:${h}px"></canvas>` +
    `<svg style="position:absolute;inset:0" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${s}</svg>`;
}
function scopeGeom(o, w, h) {
  return { wx0: o.wx0 ?? 44, wx1: o.wx1 ?? 680, wy0: o.wy0 ?? 20, wy1: o.wy1 ?? 222, split: o.split ?? 701, vx: o.vx ?? 894, vy: o.vy ?? 122, vr: o.vr ?? 110 };
}
const DIG = '0123456789abcdefghijklmnopqrstuvwxyz';
// After-mount painters (canvas): called by the render loop right after a panel's markup lands.
export const POST = {
  scopes(el, { spec, data, w, h }) {
    const cv = el.querySelector('canvas.scopecv'); if (!cv || !data.wave) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr);
    const c = cv.getContext('2d'); c.scale(dpr, dpr);
    const g = scopeGeom(spec.opts || {}, w, h);
    const cols = data.wave.cols, nb = data.wave.bins, cw = (g.wx1 - g.wx0) / cols.length, bh = (g.wy1 - g.wy0) / nb;
    const paint = (alpha) => {
      for (let i = 0; i < cols.length; i++) {
        const col = cols[i];
        for (let b = 0; b < nb; b++) {
          const v = DIG.indexOf(col[b]); if (v <= 0) continue;
          c.fillStyle = `rgba(236,214,196,${(v / 35 * alpha).toFixed(3)})`;
          c.fillRect(g.wx0 + i * cw, g.wy1 - (b + 1) * bh, cw + .4, bh + .4);
        }
      }
    };
    c.globalCompositeOperation = 'lighter';
    c.filter = 'blur(2px)'; paint(.35); c.filter = 'none'; paint(.72);
    c.globalCompositeOperation = 'source-over';
    // vectorscope
    const V = data.vec || {}, X = q => g.vx + q[0] * g.vr, Y = q => g.vy + q[1] * g.vr;
    c.strokeStyle = '#2c2b29'; c.lineWidth = 1;
    c.beginPath(); c.moveTo(g.vx - g.vr, g.vy); c.lineTo(g.vx + g.vr, g.vy); c.moveTo(g.vx, g.vy - g.vr); c.lineTo(g.vx, g.vy + g.vr); c.stroke();
    c.strokeStyle = '#b98d77'; c.lineWidth = 2; c.beginPath(); c.arc(g.vx, g.vy, g.vr, 0, Math.PI * 2); c.stroke();
    c.filter = 'blur(3px)';
    for (const q of V.pts || []) { c.fillStyle = q[2]; c.globalAlpha = .3; c.beginPath(); c.arc(X(q), Y(q), 4.5, 0, Math.PI * 2); c.fill(); }
    c.filter = 'none'; c.globalAlpha = 1; c.globalCompositeOperation = 'source-over';
    if (V.skin) { c.strokeStyle = '#efe7da'; c.lineWidth = 2; c.beginPath(); c.moveTo(X(V.skin[0]), Y(V.skin[0])); c.lineTo(X(V.skin[1]), Y(V.skin[1])); c.stroke(); }
    c.strokeStyle = '#a88a76'; c.lineWidth = 1.3;
    for (const q of V.targets || []) { c.beginPath(); c.arc(X(q), Y(q), 3.4, 0, Math.PI * 2); c.stroke(); }
  },
};

// ---------- CIE 1931 ----------
const LOCUS = [[.1741, .0050], [.1740, .0050], [.1738, .0049], [.1736, .0049], [.1733, .0048], [.1730, .0048], [.1726, .0048], [.1721, .0048], [.1714, .0051], [.1703, .0058], [.1689, .0069], [.1669, .0086], [.1644, .0109], [.1611, .0138], [.1566, .0177], [.1510, .0227], [.1440, .0297], [.1355, .0399], [.1241, .0578], [.1096, .0868], [.0913, .1327], [.0687, .2007], [.0454, .2950], [.0235, .4127], [.0082, .5384], [.0039, .6548], [.0139, .7502], [.0389, .8120], [.0743, .8338], [.1142, .8262], [.1547, .8059], [.1929, .7816], [.2296, .7543], [.2658, .7243], [.3016, .6923], [.3373, .6589], [.3731, .6245], [.4087, .5896], [.4441, .5547], [.4788, .5202], [.5125, .4866], [.5448, .4544], [.5752, .4242], [.6029, .3965], [.6270, .3725], [.6482, .3514], [.6658, .3340], [.6801, .3197], [.6915, .3083], [.7006, .2993], [.7079, .2920], [.7140, .2859], [.7190, .2809], [.7230, .2770], [.7260, .2740], [.7283, .2717], [.7300, .2700], [.7311, .2689], [.7320, .2680], [.7327, .2673], [.7334, .2666], [.7340, .2660], [.7344, .2656], [.7346, .2654], [.7347, .2653]];
function cie({ spec, data, w, h }) {
  const o = spec.opts || {};
  const pad = o.pad ?? [16, 16, 30, 16];
  const W = w - pad[1] - pad[3], H = h - pad[0] - pad[2];
  const sc0 = Math.min(W / .8, H / .9);
  const scx = o.scx ?? sc0, scy = o.scy ?? sc0;
  const ox = o.ox ?? pad[3] + (W - .8 * scx) / 2, oy = o.oy ?? pad[0] + .9 * scy;
  const P = ([x, y]) => [px(ox + x * scx), px(oy - y * scy)];
  const path = LOCUS.map((p, i) => (i ? 'L' : 'M') + P(p).join(' ')).join('') + 'Z';
  const bx = ox, by = oy - .9 * scy, bw = .8 * scx, bh = .9 * scy;
  let s = `<defs><clipPath id="hs"><path d="${path}"/></clipPath>
    <linearGradient id="cx" x1="0" x2="1" y1="0" y2="0"><stop offset="0" stop-color="#2a3cff"/><stop offset=".35" stop-color="#25c8d0"/><stop offset=".55" stop-color="#7fe05a"/><stop offset=".75" stop-color="#f3d23c"/><stop offset="1" stop-color="#ff3a2a"/></linearGradient>
    <linearGradient id="cy" x1="0" x2="0" y1="1" y2="0"><stop offset="0" stop-color="#5a2bff" stop-opacity=".9"/><stop offset=".5" stop-color="#ffffff" stop-opacity=".0"/><stop offset="1" stop-color="#30ff60" stop-opacity=".7"/></linearGradient>
    <radialGradient id="cw" cx="${px(P([.3127, .329])[0])}" cy="${px(P([.3127, .329])[1])}" r="${px(.16 * scx)}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#fff" stop-opacity=".4"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient></defs>`;
  if (o.grid) { for (let v = 0; v <= .9001; v += .1) { const [x] = P([v, 0]), [, y] = P([0, v]); if (v <= .8001) s += `<line x1="${x}" x2="${x}" y1="${px(by)}" y2="${px(oy)}" stroke="${o.gridC || '#24241f'}"/>`; s += `<line x1="${px(bx)}" x2="${px(bx + (o.gridW ?? bw))}" y1="${y}" y2="${y}" stroke="${o.gridC || '#24241f'}"/>`; } }
  s += `<g clip-path="url(#hs)" opacity="${o.fillOp ?? .85}"><rect x="${bx}" y="${by}" width="${bw}" height="${bh}" fill="url(#cx)"/><rect x="${bx}" y="${by}" width="${bw}" height="${bh}" fill="url(#cy)"/>${o.white ? `<rect x="${bx}" y="${by}" width="${bw}" height="${bh}" fill="url(#cw)"/>` : ''}</g>`;
  s += `<path d="${path}" fill="none" stroke="#d9d4c9" stroke-width="1"/>`;
  for (const g of (data.gamuts || [])) {
    const lp = g.lpt ? P(g.lpt) : [P(g.pts[1])[0] + 6, P(g.pts[1])[1] + 2];
    s += `<polygon points="${g.pts.map(p => P(p).join(',')).join(' ')}" fill="none" stroke="${g.color || 'var(--cream)'}" stroke-width="${g.w ?? 1.3}"${g.dash ? ' stroke-dasharray="3 3"' : ''}/>` + (g.label ? `<text x="${lp[0]}" y="${lp[1]}" font-size="${o.gFs ?? 9.5}" fill="var(--text)">${E(g.label)}</text>` : '');
  }
  for (const p of (data.points || [])) s += p[2] === 'ring' ? `<circle cx="${P(p)[0]}" cy="${P(p)[1]}" r="3" fill="none" stroke="#f4f0e8" stroke-width="1.4"/>` : `<circle cx="${P(p)[0]}" cy="${P(p)[1]}" r="3.5" fill="var(--ember)"/>`;
  s += `<text x="${o.labX ?? w / 2}" y="${h - (o.labB ?? 12)}" font-size="${o.labFs ?? 10.5}" fill="var(--dim)" text-anchor="middle" font-family="${o.labFont ?? 'inherit'}">${E(data.label || 'CIE 1931')}</text>`;
  return head(spec, data) + `<svg style="position:absolute;inset:0" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${s}</svg>`;
}

// ---------- loudness ----------
function loudness({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  html += `<div style="position:absolute;left:15px;top:${o.bigY ?? 44}px;font-family:var(--mono);font-weight:600;font-size:${o.bigFs ?? 30}px;letter-spacing:-.02em" class="cream">${E(data.lufs)}<span style="font-size:13px;font-weight:400;margin-left:6px" class="dim">LUFS</span></div>`;
  html += `<div style="position:absolute;left:15px;top:${(o.bigY ?? 44) + 44}px;font-family:var(--mono);font-size:11.5px" class="dim">target ${E(data.target)}</div>`;
  html += `<div style="position:absolute;left:15px;right:${o.meterW ?? 70}px;top:${(o.bigY ?? 44) + 66}px;height:1px;background:var(--line)"></div>`;
  (data.rows || []).forEach((r, i) => html += `<div style="position:absolute;left:15px;right:${(o.meterW ?? 70) + 4}px;top:${(o.bigY ?? 44) + 76 + i * 26}px;display:flex;justify-content:space-between;font-size:12px"><span class="dim">${E(r[0])}</span><span class="mono">${E(r[1])}</span></div>`);
  const mx = w - (o.meterW ?? 70) + 10, my0 = o.my0 ?? 40, my1 = h - 30;
  (data.meters || []).forEach((m, i) => {
    const x = mx + i * 22, H = my1 - my0;
    html += `<div style="position:absolute;left:${x}px;top:${my0}px;width:12px;height:${H}px;background:#161615;border-radius:2px;overflow:hidden"><div style="position:absolute;left:0;right:0;bottom:0;height:${m.v * 100}%;background:linear-gradient(#e8e3d6,#bdb8ae)"></div><div style="position:absolute;left:0;right:0;height:2px;bottom:${m.peak * 100}%;background:var(--ember)"></div></div><div class="mono dim" style="position:absolute;left:${x}px;width:12px;text-align:center;top:${my1 + 6}px;font-size:10.5px">${E(m.l)}</div>`;
  });
  return html;
}

// ---------- grading bottom: wheels + parade ----------
function wheels({ spec, data, w, h }) {
  const o = spec.opts || {};
  const pw = o.paradeW ?? Math.round(w * .36);
  const ww = w - pw, n = (data.wheels || []).length || 3;
  const r = o.r ?? Math.min(ww / n / 2 - 22, (h - 70) / 2);
  let s = `<defs><radialGradient id="whc" cx="50%" cy="50%" r="50%"><stop offset="0" stop-color="#1b1a19"/><stop offset="1" stop-color="#121211"/></radialGradient>
   <linearGradient id="ring" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#7a5ad8"/><stop offset=".35" stop-color="#3fa3a0"/><stop offset=".7" stop-color="#9fbf6a"/><stop offset="1" stop-color="#e8c79a"/></linearGradient>
   <linearGradient id="ringE" x1="0" y1="1" x2="1" y2="0"><stop offset="0" stop-color="#e8c79a"/><stop offset="1" stop-color="#ff5a23"/></linearGradient></defs>`;
  (data.wheels || []).forEach((W, i) => {
    const cx = ww / n * (i + .5), cy = (o.cy ?? (r + 34));
    s += `<text x="${cx}" y="${o.labY ?? 18}" font-size="12" fill="var(--text)" text-anchor="middle">${E(W.name)}</text>`;
    s += `<circle cx="${cx}" cy="${cy}" r="${r}" fill="url(#whc)" stroke="url(#${W.em ? 'ringE' : 'ring'})" stroke-width="3"/>`;
    s += `<line x1="${cx - r + 8}" x2="${cx + r - 8}" y1="${cy}" y2="${cy}" stroke="#262624"/><line x1="${cx}" x2="${cx}" y1="${cy - r + 8}" y2="${cy + r - 8}" stroke="#262624"/>`;
    const [dx, dy] = W.dot || [0, 0];
    s += `<circle cx="${px(cx + dx * r)}" cy="${px(cy - dy * r)}" r="4" fill="${W.em ? 'var(--ember)' : 'var(--cream)'}"/>`;
    if (W.ring != null) { const a = W.ring * Math.PI / 180; s += `<circle cx="${px(cx + Math.cos(a) * r)}" cy="${px(cy - Math.sin(a) * r)}" r="4.5" fill="var(--ember)"/>`; }
    s += `<text x="${cx}" y="${cy + r + 22}" font-size="11.5" text-anchor="middle">` + W.vals.map((v, k) => `<tspan dx="${k ? 10 : 0}" fill="${(W.em && k === 0) ? 'var(--ember-text)' : (W.cols && W.cols[k]) || 'var(--text)'}">${E(v)}</tspan>`).join('') + '</text>';
  });
  // parade
  const x0 = ww + 34, x1 = w - 12, y0 = o.py0 ?? 22, y1 = h - (o.pyb ?? 22);
  s += `<line x1="${ww}" x2="${ww}" y1="14" y2="${h - 14}" stroke="var(--line)"/>`;
  s += [1023, 768, 512, 256, 0].map((v, i) => `<text x="${x0 - 6}" y="${px(y0 + i * (y1 - y0) / 4 + 3)}" font-size="9.5" fill="var(--dim)" text-anchor="end">${v}</text><line x1="${x0}" x2="${x1}" y1="${px(y0 + i * (y1 - y0) / 4)}" y2="${px(y0 + i * (y1 - y0) / 4)}" stroke="#1a1a19"/>`).join('');
  const cw = (x1 - x0) / 3;
  const cols = ['#e2553f', '#5fbf6a', '#4f7fe0'], lifts = [.05, 0, -.04];
  cols.forEach((c, k) => s += `<svg x="${x0 + k * cw + 3}" y="${y0}" width="${cw - 6}" height="${y1 - y0}">${lumaTrace(cw - 6, y1 - y0, 11 + k, c, .55, u => .42 + lifts[k] + .2 * Math.sin(u * 5 + k))}</svg>`);
  return head(spec, data) + `<svg style="position:absolute;inset:0" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${s}</svg>`;
}

// ---------- node stack (modelling) ----------
function stack({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  const y0 = o.y0 ?? 56, step = o.step ?? 64, cw = o.cw ?? 150;
  (data.nodes || []).forEach((n, i) => {
    const y = y0 + i * step;
    if (i) html += `<div style="position:absolute;left:${15 + cw / 2}px;top:${y - step + 30}px;width:1px;height:${step - 30}px;background:var(--faint)"></div>`;
    html += `<div style="position:absolute;left:15px;top:${y}px;width:${cw}px;height:30px;border:1px solid ${n.em ? 'var(--ember)' : 'var(--line)'};border-radius:4px;background:${n.em ? 'var(--ember-tint)' : 'var(--raise)'};display:flex;align-items:center;padding:0 10px;font-family:var(--mono);font-size:12px;color:var(--cream)">${E(n.name)}</div>`;
  });
  return html;
}
// ---------- profile / history (modelling) ----------
function profile({ spec, data, w, h }) {
  const o = spec.opts || {};
  const gw = o.gw ?? Math.round(w * .38);
  let s = `<line x1="40" x2="40" y1="40" y2="${h - 14}" stroke="#2a2a28"/>`;
  const pts = (data.pts || []).map(([x, y]) => [40 + x * (gw - 60), h - 18 - y * (h - 60)]);
  s += `<path d="${pts.map((p, i) => (i ? 'L' : 'M') + px(p[0]) + ' ' + px(p[1])).join('')}" fill="none" stroke="var(--cream)" stroke-width="1.4"/>`;
  pts.forEach(p => s += `<path d="M${px(p[0])} ${px(p[1] - 4)} l4 4 l-4 4 l-4 -4z" fill="var(--cream)"/>`);
  if (data.cursor != null) { const x = 40 + data.cursor * (gw - 60); s += `<line x1="${x}" x2="${x}" y1="36" y2="${h - 12}" stroke="#8a867d"/>`; }
  let html = head(spec, data) + `<svg style="position:absolute;inset:0" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${s}</svg>`;
  const items = data.thumbs || []; const tx0 = gw + 20, tw = (w - tx0 - 15 - 12 * (items.length - 1)) / Math.max(1, items.length);
  items.forEach((t, i) => html += `<div style="position:absolute;left:${px(tx0 + i * (tw + 12))}px;width:${px(tw)}px;top:${o.ty ?? 36}px;height:${o.th ?? 74}px" class="thumb ${t.sel ? 'sel' : ''}">${t.src ? `<img src="${E(t.src)}">` : ''}</div><div class="mono dim" style="position:absolute;left:${px(tx0 + i * (tw + 12))}px;width:${px(tw)}px;top:${(o.ty ?? 36) + (o.th ?? 74) + 6}px;text-align:center;font-size:11px">${E(t.label)}</div>`);
  return html;
}
// ---------- simple list rows (lights, emitters) ----------
function list({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  (data.rows || []).forEach((r, i) => {
    const y = (o.y0 ?? 44) + i * ((o.rowH ?? 30) + (o.gap ?? 0));
    html += `<div style="position:absolute;left:${o.chips ? 10 : 15}px;right:${o.chips ? 10 : 15}px;top:${y}px;height:${o.rowH ?? 30}px;display:flex;align-items:center;gap:10px;font-size:12.5px;padding:0 ${o.chips ? 9 : 0}px;${o.chips ? 'background:var(--raise);border-radius:4px;' : ''}${o.lines ? 'border-bottom:1px solid var(--line2);' : ''}">` +
      (r.icon ? `<span style="width:16px;text-align:center;color:${r.dim ? 'var(--faint)' : 'var(--cream)'}">${r.icon === 'bulb' ? '<svg width="12" height="14" viewBox="0 0 12 14"><path d="M6 1a4.5 4.5 0 0 0-2.6 8.2V11h5.2V9.2A4.5 4.5 0 0 0 6 1z" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M4 12.8h4" stroke="currentColor" stroke-width="1.2"/></svg>' : E(r.icon)}</span>` : '') +
      `<span style="font-family:var(--label-font)" class="${r.dim ? 'dim' : 'cream'}">${E(r.k)}</span><span class="mono ${r.em ? 'em' : 'dim'}" style="margin-left:auto">${E(r.v)}</span></div>`;
  });
  return html;
}
// ---------- scene tree ----------
function tree({ spec, data, w, h }) {
  const o = spec.opts || {};
  let html = head(spec, data);
  const ic = { cam: '<path d="M1 4h8v6H1z M9 6l4-2v6l-4-2" fill="none" stroke="currentColor" stroke-width="1.1"/>', light: '<circle cx="6" cy="6" r="3" fill="none" stroke="currentColor" stroke-width="1.1"/><path d="M6 0v2M6 10v2M0 6h2M10 6h2" stroke="currentColor" stroke-width="1.1"/>', mesh: '<path d="M6 1l5 3v5l-5 3-5-3V4z" fill="none" stroke="currentColor" stroke-width="1.1"/>', plane: '<path d="M1 9l4-5h7l-4 5z" fill="none" stroke="currentColor" stroke-width="1.1"/>' };
  (data.items || []).forEach((it, i) => {
    const y = (o.y0 ?? 44) + i * (o.rowH ?? 30);
    html += `<div style="position:absolute;left:10px;right:10px;top:${y}px;height:${(o.rowH ?? 30) - 4}px;display:flex;align-items:center;gap:9px;padding-left:${10 + (it.d || 0) * 16}px;border-radius:4px;${it.sel ? 'background:var(--raise2);' : ''}font-family:var(--mono);font-size:12px">` +
      `<svg width="14" height="12" viewBox="0 0 14 12" style="color:${it.sel ? 'var(--cream)' : 'var(--dim)'}">${ic[it.icon] || ''}</svg><span class="${it.sel ? 'cream' : 'text'}" style="color:${it.sel ? 'var(--cream)' : 'var(--text)'}">${E(it.name)}</span></div>`;
  });
  return html;
}
// ---------- light rig (lighting) ----------
function lightrig({ spec, data, w, h }) {
  const o = spec.opts || {};
  const dw = o.dw ?? 190, tw = o.tw ?? 250;
  let s = '';
  const cx = 15 + dw / 2, cy = (h + 20) / 2, R = Math.min(dw / 2 - 10, (h - 70) / 2);
  s += `<circle cx="${cx}" cy="${cy}" r="${R}" fill="none" stroke="#2a2a28"/><circle cx="${cx}" cy="${cy}" r="${R * .55}" fill="none" stroke="#1d1d1c"/>`;
  s += `<rect x="${cx - 6}" y="${cy - 10}" width="12" height="20" rx="3" fill="#8a867d"/>`;
  for (const L of (data.lights || [])) {
    const a = L.a * Math.PI / 180, x = cx + Math.cos(a) * R, y = cy - Math.sin(a) * R;
    s += `<line x1="${px(x)}" y1="${px(y)}" x2="${cx}" y2="${cy}" stroke="${L.em ? 'var(--ember)' : '#5a5650'}" stroke-dasharray="2 3"/><circle cx="${px(x)}" cy="${px(y)}" r="6" fill="${L.em ? 'var(--ember)' : 'var(--cream)'}"/><text x="${px(x + (Math.cos(a) >= 0 ? 10 : -10))}" y="${px(y + 4)}" font-size="10.5" fill="var(--dim)" text-anchor="${Math.cos(a) >= 0 ? 'start' : 'end'}">${E(L.name)}</text>`;
  }
  let html = head(spec, data) + `<svg style="position:absolute;inset:0" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${s}</svg>`;
  const tx = 15 + dw + 30, tr = w - (o.thumbsW ?? 250) - 30;
  const cols = 'grid-template-columns:1.2fr 1fr 1.4fr .7fr';
  html += `<div class="tbl" style="left:${tx}px;right:${w - tr}px;top:${o.ty ?? 44}px"><div class="tr h" style="${cols}"><span>LIGHT</span><span>TEMP</span><span>INTENSITY</span><span style="text-align:right">FALLOFF</span></div>` +
    (data.rows || []).map(r => `<div class="tr" style="${cols};height:34px"><span class="mono">${E(r[0])}</span><span class="mono dim">${E(r[1])}</span><span style="display:flex;align-items:center;gap:8px"><span style="flex:1;height:2px;background:#2a2a28;position:relative"><i style="position:absolute;left:0;top:0;bottom:0;width:${r[3] * 100}%;background:${r[4] ? 'var(--ember)' : '#bdb8ae'}"></i></span><span class="mono" style="width:3ch;text-align:right">${E(r[2])}</span></span><span class="mono dim" style="text-align:right">${E(r[5])}</span></div>`).join('') + '</div>';
  const th = data.thumbs || []; const gx = w - (o.thumbsW ?? 250) - 15, gw = (o.thumbsW ?? 250), cw = (gw - 10) / 2, chh = (h - 40 - 10) / 2;
  th.forEach((t, i) => { const x = gx + (i % 2) * (cw + 10), y = 20 + Math.floor(i / 2) * (chh + 10); html += `<div class="thumb ${t.sel ? 'sel' : ''}" style="position:absolute;left:${px(x)}px;top:${px(y)}px;width:${px(cw)}px;height:${px(chh)}px">${t.src ? `<img src="${E(t.src)}">` : ''}<span class="mono" style="position:absolute;left:6px;bottom:4px;font-size:10.5px;color:var(--cream)">${E(t.label)}</span></div>`; });
  return html;
}
// ---------- renders table (export) ----------
function renders({ spec, data, w, h }) {
  const o = spec.opts || {};
  const cols = `grid-template-columns:${o.cols || '2.2fr 1.6fr 1fr 1.6fr 1fr'}`;
  let html = head(spec, data);
  html += `<div class="tbl" style="top:${o.top ?? 42}px">` + `<div class="tr h" style="${cols}">${(data.head || []).map(x => `<span>${E(x)}</span>`).join('')}</div>` +
    (data.rows || []).map(r => `<div class="tr" style="${cols};height:${o.rowH ?? 30}px">` + r.map((c, i) => {
      if (i === 3) { const st = String(c); const em = st.startsWith('●'); return `<span class="mono ${em ? 'em' : 'dim'}">${E(st)}</span>`; }
      return `<span class="${i ? 'mono dim' : 'mono cream'}">${E(c)}</span>`;
    }).join('') + '</div>').join('') + '</div>';
  if (data.foot) html += `<div class="pfoot" style="left:auto;right:15px">${E(data.foot)}</div>`;
  return html;
}

export const KINDS = { agent, viewer, kv, timeline, keys, curves, transcript, history, diffbatch, media, scopes, cie, loudness, wheels, stack, profile, list, tree, lightrig, renders };
