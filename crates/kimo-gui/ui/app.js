// Kimo spectator frontend. Read-only: receives engine tool events, derives
// the call log, pipeline graph, pass stack, inspector and timeline from them.
// When no Tauri shell is present (Chromium dev harness) a simulated feed runs
// and the status pill says so - simulated data is always labeled.
import { markDirty, setCubeColor } from './viewer.js';
import { connect } from './bridge.js';

const $ = s => document.querySelector(s);
const PALETTE = ['#c9b99a','#7fb7a6','#b9a6e8','#e889a0','#a7948d','#9a8f5f'];  // no ember: it is reserved for active/highlight
const colorFor = name => PALETTE[[...name].reduce((a,c)=>(a*31+c.charCodeAt(0))>>>0,7) % PALETTE.length];

const state = {
  rev: 0, calls: [],            // ring buffer, capped (bounded memory under bursts)
  CALLS_CAP: 400,
  nodes: new Map(),             // name -> {name,x,y,inputs:[{port,from}],active,args,dur}
  ends: [],                     // recent tool_end times for the rate pill
  focus: null,
};

/* ---------- ingestion ---------- */
function onEvent(ev) {
  if (ev.type === 'tool_start') {
    state.calls.push({ id:ev.id, name:ev.name, active:true });
    if (state.calls.length > state.CALLS_CAP) state.calls.splice(0, state.calls.length - state.CALLS_CAP);
    // pipeline membership: the node graph is derived from data dependencies -
    // a tool enters it when it consumes another tool's output or the engine
    // assigns it a slot. Pure calls (search_tools, add_clip) stay log-only.
    if (ev.inputs || ev.x != null || state.nodes.has(ev.name)) {
      const n = state.nodes.get(ev.name) || { name:ev.name, inputs:[], x:0, y:0 };
      n.active = true; n.args = ev.args || n.args;
      if (ev.inputs) n.inputs = ev.inputs;
      if (ev.x != null) { n.x = ev.x; n.y = ev.y; }
      state.nodes.set(ev.name, n);
      for (const inp of (ev.inputs||[])) {
        const srcName = inp.from.split(':')[0];
        if (!state.nodes.has(srcName)) state.nodes.set(srcName, { name:srcName, inputs:[], x:0, y:0 });
      }
      state.focus = ev.name;
    }
  } else if (ev.type === 'tool_end') {
    const c = [...state.calls].reverse().find(c => c.id === ev.id);
    if (c) { c.active = false; c.dur = ev.dur_ms; c.status = ev.status; }
    const n = state.nodes.get(ev.name);
    if (n) { n.active = false; n.dur = ev.dur_ms; }
    state.ends.push(Date.now());
    markDirty();
  } else if (ev.type === 'project_changed') {
    state.rev = ev.rev;
    state.calls.push({ sys:`project_changed  rev ${ev.rev}` });
    if (state.calls.length > state.CALLS_CAP) state.calls.splice(0, state.calls.length - state.CALLS_CAP);
  } else if (ev.type === 'frame_rendered') {
    $('#frame-no').textContent = ev.frame;
    if (ev.png_b64) blitEngineFrame(ev.png_b64);
  } else if (ev.type === 'timeline') {
    if (Array.isArray(ev.tracks)) buildTracks(ev.tracks);
  } else if (ev.type === 'viewport_tint') {
    setCubeColor(ev.color);
  }
  scheduleRender();
}

/* Burst discipline: events only mutate state; the DOM is touched once per
   frame at most, so a 500-event burst costs one layout, not five hundred. */
let renderQueued = false;
function scheduleRender() {
  if (renderQueued) return;
  renderQueued = true;
  requestAnimationFrame(() => { renderQueued = false; render(); });
}

/* ---------- render ---------- */
const logEl = $('#log-rows');
const LOG_CAP = 200;                      // ring buffer: bounded DOM + memory under bursts
const logRows = new Map();                // call id -> row element
function logAppend(id) {
  const row = document.createElement('div');
  row.className = 'log-row';
  row.innerHTML = `<span></span><span class="dur"></span>`;
  logEl.appendChild(row);
  while (logEl.children.length > LOG_CAP) {
    const first = logEl.firstChild; first.remove();
    for (const [k,v] of logRows) if (v === first) { logRows.delete(k); break; }
  }
  if (id) logRows.set(id, row);
  return row;
}
function render() {
  for (const c of state.calls) {
    if (c.rendered) continue;
    if (c.sys) {
      const row = logAppend(null); row.classList.add('sys'); row.firstChild.textContent = c.sys;
      c.rendered = true; continue;
    }
    let row = c.id && logRows.get(c.id);
    if (!row) { row = logAppend(c.id); }
    row.classList.toggle('active', !!c.active);
    row.firstChild.innerHTML = `${c.active?'tool_start':'tool_end'}&nbsp; <span class="tname">${c.name}</span>`;
    row.lastChild.textContent = c.dur!=null ? c.dur+' ms' : (c.active?'…':'');
    if (!c.active) c.rendered = true;   // tool_end is the row's final state
  }
  logEl.scrollTop = logEl.scrollHeight;   // stay pinned to the newest row
  $('#rev-pill').textContent = 'rev ' + state.rev;
  const minute = state.ends.filter(t => Date.now()-t < 60000).length;
  $('#rate-pill').textContent = `tools ${minute}/min`;
  renderGraph(); renderPasses(); renderInspector();
}

/* derived graph: nodes from tool_start inputs, read-only, live path animated.
   Pills are appended first and measured, then bezier splines anchor to the
   real port dots, so edges stay glued whatever the pill width. */
function renderGraph() {
  const nodesEl = $('#nodes'), edgesEl = $('#edges');
  const nodes = [...state.nodes.values()];
  // simple layered layout: BFS depth from sources, stable per name
  const depth = n => { let d=0,g=new Set(),cur=n; while(cur.inputs.length&&cur.inputs[0].from&&!g.has(cur.name)){g.add(cur.name);cur=state.nodes.get(cur.inputs[0].from.split(':')[0])||{inputs:[]};d++;} return d; };
  const lanes = {};
  for (const n of nodes) { const d = n.x?null:depth(n); if(!n.x){ n.x = 60 + d*220; const lane = lanes[d]=(lanes[d]||0); n.y = 40 + lane*110; lanes[d]=lane+1; } }
  nodesEl.innerHTML = ''; edgesEl.innerHTML = '';
  const box = {};
  for (const n of nodes) {
    const el = document.createElement('div');
    el.className = 'node' + (n.active ? ' active' : '');
    el.style.left = n.x+'px'; el.style.top = n.y+'px';
    el.style.setProperty('--nc', colorFor(n.name));   // identity color (dot + out port)
    el.innerHTML = `<span class="ndot"></span><span class="nname">${n.name}</span>` +
      (n.dur != null ? `<span class="ndur">${n.dur}&thinsp;ms</span>` : '') +
      n.inputs.map((_,k) => `<span class="pdot in" style="top:${8+k*12}px"></span>`).join('') +
      `<span class="pdot out"></span>`;
    nodesEl.appendChild(el);
    box[n.name] = el;
  }
  const SVGNS = 'http://www.w3.org/2000/svg';
  for (const n of nodes) {
    for (const inp of n.inputs) {
      const src = state.nodes.get(inp.from.split(':')[0]);
      if (!src || !box[src.name]) continue;
      const a = box[src.name], b = box[n.name];
      const x1 = src.x + a.offsetWidth, y1 = src.y + a.offsetHeight/2;
      const x2 = n.x, y2 = n.y + b.offsetHeight/2, mx = (x1+x2)/2;
      const d = `M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`;
      const live = n.active || src.active;
      if (live) {   // cheap glow: wide translucent twin under the live spline
        const g = document.createElementNS(SVGNS,'path');
        g.setAttribute('d', d); g.setAttribute('class', 'edge-glow live');
        edgesEl.appendChild(g);
      }
      const p = document.createElementNS(SVGNS,'path');
      p.setAttribute('d', d);
      p.setAttribute('class', 'edge' + (live ? ' live' : ''));
      edgesEl.appendChild(p);
    }
  }
  // presenter mode: keep the active pill in view
  const act = nodesEl.querySelector('.node.active');
  if (act) act.scrollIntoView({ block:'nearest', inline:'nearest', behavior:'smooth' });
}

function renderPasses() {
  const done = [...state.nodes.values()].filter(n=>n.dur!=null).sort((a,b)=>a.x-b.x).slice(-6);
  $('#pass-rows').innerHTML = done.map((n,i)=>
    `<div class="pass-row"><span class="pname"><span class="idx">${String(i+1).padStart(2,'0')}</span><span class="sw" style="background:${colorFor(n.name)}"></span>${n.name.replace(/_/g,' ')}</span><span class="dur">${n.dur>0?n.dur+' ms':'— ms'}</span></div>`).join('');
}
function renderInspector() {
  const n = state.focus && state.nodes.get(state.focus);
  $('#insp-name').textContent = n ? n.name.toUpperCase() : '—';
  const rows = n && n.args ? Object.entries(n.args).map(([k,v])=>`<div class="insp-row"><span>${k}</span><span class="val">${v}</span></div>`) : [];
  if (n && n.dur!=null) rows.push(`<div class="insp-gap"></div><div class="insp-row"><span>dur</span><span class="val">${n.dur} ms</span></div>`);
  $('#insp-rows').innerHTML = rows.join('') || '<div class="insp-row"><span class="dim">no selection</span></div>';
}

/* engine frame: blit a real frame payload; without one the card keeps its
   placeholder gradient + tagline (labeled by the feed state, as in mockup A) */
async function blitEngineFrame(b64) {
  // decode off the UI thread; only the draw lands on the main thread
  const blob = await (await fetch('data:image/png;base64,' + b64)).blob();
  const bmp = await createImageBitmap(blob);
  const c = $('#engine-canvas'), r = c.parentElement.getBoundingClientRect();
  c.width = r.width; c.height = r.height;
  const g = c.getContext('2d');
  const s = Math.max(c.width/bmp.width, c.height/bmp.height);
  g.drawImage(bmp, (c.width-bmp.width*s)/2, (c.height-bmp.height*s)/2, bmp.width*s, bmp.height*s);
  bmp.close();
  $('.tagline').style.display = 'none';
}

/* ---------- timeline (DOM-only playhead, zero GPU) ---------- */
const TL0 = 210, PXS = 48;               // one time->x scale: ruler, bars and playhead share it
const tx = s => TL0 + s*PXS;
const TRACKS = [                          // times in seconds
  { label:'V3 glass_panel', color:'#c9b99a', t0:4,   t1:10,   kfs:[4.5,6,7.5] },
  { label:'V2 title',       color:'#9a8f5f', t0:4,   t1:13,   kfs:[] },
  { label:'V1 plate.mp4',   color:'#7fb7a6', t0:2.5, t1:17,   kfs:[] },
  { label:'A1 score.wav',   color:'#b9a6e8', t0:2.5, t1:18.5, kfs:[] },
];
function buildTracks(tracks) {
  const el = $('#tl-tracks'); el.innerHTML = '';
  for (const tr of tracks) {
    const row = document.createElement('div'); row.className='track';
    row.innerHTML = `<span class="tlabel">${tr.label}</span>
      <span class="bar" style="left:${tx(tr.t0)}px;width:${(tr.t1-tr.t0)*PXS}px;background:${tr.color}"></span>` +
      (tr.kfs||[]).map(x=>`<span class="kf" style="left:${tx(x)}px"></span>`).join('');
    el.appendChild(row);
  }
}
function buildTimeline() {
  const ruler = $('#tl-ruler');
  for (let s=0; s<=20; s+=2) {
    const t = document.createElement('span'); t.className='tick';
    t.style.left = tx(s)+'px'; t.textContent = '00:'+String(s).padStart(2,'0');
    ruler.appendChild(t);
  }
  buildTracks(TRACKS);  // fallback demo set until a timeline event arrives
  const ph = $('#playhead'); const t0 = performance.now();
  (function tick(){
    const el = (performance.now()-t0)/1000;
    if (el > 20) { ph.style.left = tx(20)+'px'; $('#tc').textContent = '00:20.0 · 24 FPS'; return; }  // park at the end, zero idle
    ph.style.left = tx(el)+'px';
    $('#tc').textContent = `00:${String(Math.floor(el)).padStart(2,'0')}.${Math.floor((el%1)*10)} · 24 FPS`;
    requestAnimationFrame(tick);
  })();
}
buildTimeline();


/* ---------- workstation chrome ---------- */
/* collapsible docks: every dock-toggle flips .collapsed on its target panel */
document.addEventListener('click', e => {
  const b = e.target.closest('.dock-toggle'); if (!b) return;
  const d = document.getElementById(b.dataset.dock);
  if (d) d.classList.toggle('collapsed');
});
/* floating inspector: drag by its header (pointer events, transform-free) */
{ const insp = $('#inspector'), head = $('#insp-head'), host = $('main');
  head.addEventListener('pointerdown', e => {
    if (e.target.closest('.dock-toggle')) return;
    const r = insp.getBoundingClientRect(), hr = host.getBoundingClientRect();
    if (!insp.classList.contains('floating')) {   // undock: float at the current rect
      insp.classList.add('floating');
      insp.style.left = (r.left - hr.left) + 'px'; insp.style.top = (r.top - hr.top) + 'px';
    }
    const ox = e.clientX - r.left, oy = e.clientY - r.top;
    const move = ev => { insp.style.left = (ev.clientX - hr.left - ox) + 'px'; insp.style.top = (ev.clientY - hr.top - oy) + 'px'; };
    const up = () => { removeEventListener('pointermove', move); removeEventListener('pointerup', up); };
    addEventListener('pointermove', move); addEventListener('pointerup', up);
  }); }

/* ---------- feeds ---------- */
function setPill(txt, sim) {
  const p = $('#conn-pill'); p.textContent = txt; p.classList.toggle('sim', !!sim);
  $('#feed-label').textContent = sim ? 'simulated feed' : 'live feed';
}
// Transport selection lives in bridge.js; status strings stay honest about
// where the data comes from (tauri ipc / sse loopback / simulated).
const binding = connect(onEvent, (st, detail) => {
  if (st === 'live') setPill(`● live · ${detail}`, false);
  else if (st === 'sim') setPill('● dev · simulated feed', true);
  else setPill(`● ${detail}`, false);
}, () => { setPill('● dev · simulated feed', true); runSimulatedFeed(); });
if (binding.kind === 'tauri') setPill('● connecting · loopback :7421', false);

/* Procedural stand-in for an engine frame (sim feed only): a real PNG drawn
   on an offscreen canvas, so the blit path is exercised end-to-end. */
function demoFramePng() {
  const c = document.createElement('canvas'); c.width = 640; c.height = 360;
  const g = c.getContext('2d');
  const grad = g.createLinearGradient(0,0,640,360);
  grad.addColorStop(0,'#ff5a23'); grad.addColorStop(.55,'#33222e'); grad.addColorStop(1,'#15151b');
  g.fillStyle = grad; g.fillRect(0,0,640,360);
  g.strokeStyle = '#f4f0e8'; g.lineWidth = 3; g.globalAlpha = .8;
  g.strokeRect(150,80,340,200); g.globalAlpha = 1;
  return c.toDataURL('image/png').split(',')[1];
}

/* Simulated feed for the Chromium dev harness: replays the mockup A scenario.
   Ships in the build; only runs when no shell is present. */
function runSimulatedFeed() {
  const seq = [
    ['tool_start',{id:'c1',name:'load_media',x:60,y:40}],
    ['tool_end',{id:'c1',name:'load_media',dur_ms:96,status:'ok'}],
    ['tool_start',{id:'c2',name:'text_layer',x:60,y:200}],
    ['tool_end',{id:'c2',name:'text_layer',dur_ms:41,status:'ok'}],
    ['tool_start',{id:'c3',name:'color_grade',x:290,y:40,inputs:[{port:'in',from:'load_media:out'}],args:{lift:'0.02 0.01 0.00',gamma:'1.04',gain:'1.10 1.02 0.96',space:'ACEScct'}}],
    ['tool_end',{id:'c3',name:'color_grade',dur_ms:14,status:'ok'}],
    ['tool_start',{id:'c4',name:'glass_shatter',x:290,y:190,inputs:[{port:'in',from:'text_layer:out'},{port:'mask',from:'load_media:out'}],args:{pieces:512,seed:7}}],
    ['tool_end',{id:'c4',name:'glass_shatter',dur_ms:212,status:'ok'}],
    ['tool_start',{id:'c5',name:'composite',x:540,y:110,inputs:[{port:'bg',from:'color_grade:out'},{port:'fg',from:'glass_shatter:out'}],args:{mode:'over'}}],
    ['tool_start',{id:'c6',name:'set_keyframe',args:{track:'V3',t:'00:06'}}],
    ['tool_end',{id:'c6',name:'set_keyframe',dur_ms:1,status:'ok'}],
    ['tool_start',{id:'c7',name:'render_frame',args:{f:142}}],
    ['viewport_tint',{color:0xff5a23}],
    ['tool_end',{id:'c7',name:'render_frame',dur_ms:380,status:'ok'}],
    ['frame_rendered',{frame:142,png_b64:demoFramePng()}],
    ['tool_end',{id:'c5',name:'composite',dur_ms:31,status:'ok'}],
    ['tool_start',{id:'c8',name:'export_mp4',x:760,y:150,inputs:[{port:'in',from:'composite:out'}]}],
    ['tool_end',{id:'c8',name:'export_mp4',dur_ms:0,status:'ok'}],
    ['tool_start',{id:'c9',name:'add_clip'}],
    ['tool_end',{id:'c9',name:'add_clip',dur_ms:2,status:'ok'}],
    ['tool_start',{id:'c10',name:'search_tools',args:{q:'grain'}}],
    ['tool_end',{id:'c10',name:'search_tools',dur_ms:3,status:'ok'}],
    ['project_changed',{rev:214}],
  ];
  onEvent({ type:'timeline', tracks:TRACKS });
  let i = 0;
  (function step(){
    if (i >= seq.length) return;
    const [type, body] = seq[i++];
    onEvent({ type, ...body });
    setTimeout(step, type==='tool_start' ? 500 : 900);
  })();
}
