// Kimo spectator frontend. Read-only: receives engine tool events, derives
// the call log, pipeline graph, pass stack, inspector and timeline from them.
// When no Tauri shell is present (Chromium dev harness) a simulated feed runs
// and the status pill says so - simulated data is always labeled.
import { markDirty, setCubeColor } from './viewer.js';
import { connect } from './bridge.js';

const $ = s => document.querySelector(s);
const PALETTE = ['#dd5227','#7fb7a6','#b9a6e8','#e889a0','#a7948d','#9a8f5f'];
const colorFor = name => PALETTE[[...name].reduce((a,c)=>(a*31+c.charCodeAt(0))>>>0,7) % PALETTE.length];

const state = {
  rev: 0, calls: [],            // {id,name,dur,status,active}
  nodes: new Map(),             // name -> {name,x,y,inputs:[{port,from}],active,args,dur}
  ends: [],                     // recent tool_end times for the rate pill
  focus: null,
};

/* ---------- ingestion ---------- */
function onEvent(ev) {
  if (ev.type === 'tool_start') {
    state.calls.push({ id:ev.id, name:ev.name, active:true });
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
  } else if (ev.type === 'frame_rendered') {
    $('#frame-no').textContent = ev.frame;
  } else if (ev.type === 'viewport_tint') {
    setCubeColor(ev.color);
  }
  render();
}

/* ---------- render ---------- */
const logEl = $('#log-rows');
function render() {
  // call log (latest at top? mockup appends downward; keep append, cap 200)
  logEl.innerHTML = '';
  for (const c of state.calls.slice(-60)) {
    const row = document.createElement('div');
    row.className = 'log-row' + (c.active ? ' active' : '') + (c.sys ? ' sys' : '');
    if (c.sys) { row.innerHTML = `<span>${c.sys}</span>`; }
    else row.innerHTML = `<span>${c.active?'tool_start':'tool_end'}&nbsp; <span class="tname">${c.name}</span></span><span class="dur">${c.dur!=null?c.dur+' ms':c.active?'…':''}</span>`;
    logEl.appendChild(row);
  }
  $('#rev-pill').textContent = 'rev ' + state.rev;
  const minute = state.ends.filter(t => Date.now()-t < 60000).length;
  $('#rate-pill').textContent = `tools ${minute}/min`;
  renderGraph(); renderPasses(); renderInspector();
}

/* derived graph: nodes from tool_start inputs, read-only, live path animated */
function renderGraph() {
  const nodesEl = $('#nodes'), edgesEl = $('#edges');
  const nodes = [...state.nodes.values()];
  // simple layered layout: BFS depth from sources, stable per name
  const depth = n => { let d=0,g=new Set(),cur=n; while(cur.inputs.length&&cur.inputs[0].from&&!g.has(cur.name)){g.add(cur.name);cur=state.nodes.get(cur.inputs[0].from.split(':')[0])||{inputs:[]};d++;} return d; };
  const lanes = {};
  for (const n of nodes) { const d = n.x?null:depth(n); if(!n.x){ n.x = 60 + d*220; const lane = lanes[d]=(lanes[d]||0); n.y = 40 + lane*110; lanes[d]=lane+1; } }
  edgesEl.innerHTML = ''; nodesEl.innerHTML = '';
  for (const n of nodes) {
    for (const inp of n.inputs) {
      const src = state.nodes.get(inp.from.split(':')[0]);
      if (!src) continue;
      const p = document.createElementNS('http://www.w3.org/2000/svg','path');
      const x1=src.x+150, y1=src.y+26, x2=n.x, y2=n.y+26, mx=(x1+x2)/2;
      p.setAttribute('d', `M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`);
      p.setAttribute('class', 'edge' + (n.active||src.active ? ' live' : ''));
      edgesEl.appendChild(p);
    }
    const el = document.createElement('div');
    el.className = 'node' + (n.active ? ' active' : '');
    el.style.left = n.x+'px'; el.style.top = n.y+'px';
    el.innerHTML = `<div class="nhead" style="background:${colorFor(n.name)}">${n.name}</div>
      <div class="nbody">${n.inputs.map(i=>`<div class="port"><span>${i.port}</span></div>`).join('')}<div class="port"><span class="dim">${n.dur!=null?n.dur+' ms':''}</span><span class="dot"></span></div></div>`;
    nodesEl.appendChild(el);
  }
  // presenter mode: keep the active card in view
  const act = nodesEl.querySelector('.node.active');
  if (act) act.scrollIntoView({ block:'nearest', inline:'nearest', behavior:'smooth' });
}

function renderPasses() {
  const done = [...state.nodes.values()].filter(n=>n.dur!=null).sort((a,b)=>a.x-b.x).slice(-6);
  $('#pass-rows').innerHTML = done.map(n=>
    `<div class="pass-row"><span class="pname"><span class="sw" style="background:${colorFor(n.name)}"></span>${n.name.replace(/_/g,' ')}</span><span class="dur">${n.dur>0?n.dur+' ms':'— ms'}</span></div>`).join('');
}
function renderInspector() {
  const n = state.focus && state.nodes.get(state.focus);
  $('#insp-name').textContent = n ? n.name.toUpperCase() : '—';
  const rows = n && n.args ? Object.entries(n.args).map(([k,v])=>`<div class="insp-row"><span>${k}</span><span class="val">${v}</span></div>`) : [];
  if (n && n.dur!=null) rows.push(`<div class="insp-gap"></div><div class="insp-row"><span>dur</span><span class="val">${n.dur} ms</span></div>`);
  $('#insp-rows').innerHTML = rows.join('') || '<div class="insp-row"><span class="dim">no selection</span></div>';
}

/* ---------- timeline (DOM-only playhead, zero GPU) ---------- */
const TL0 = 210, PXS = 48;               // one time->x scale: ruler, bars and playhead share it
const tx = s => TL0 + s*PXS;
const TRACKS = [                          // times in seconds
  { label:'V3 glass_panel', color:'#dd5227', t0:4,   t1:10,   kfs:[4.5,6,7.5] },
  { label:'V2 title',       color:'#9a8f5f', t0:4,   t1:13,   kfs:[] },
  { label:'V1 plate.mp4',   color:'#7fb7a6', t0:2.5, t1:17,   kfs:[] },
  { label:'A1 score.wav',   color:'#b9a6e8', t0:2.5, t1:18.5, kfs:[] },
];
function buildTimeline() {
  const ruler = $('#tl-ruler');
  for (let s=0; s<=20; s+=2) {
    const t = document.createElement('span'); t.className='tick';
    t.style.left = tx(s)+'px'; t.textContent = '00:'+String(s).padStart(2,'0');
    ruler.appendChild(t);
  }
  const tracks = $('#tl-tracks');
  for (const tr of TRACKS) {
    const el = document.createElement('div'); el.className='track';
    el.innerHTML = `<span class="tlabel">${tr.label}</span>
      <span class="bar" style="left:${tx(tr.t0)}px;width:${(tr.t1-tr.t0)*PXS}px;background:${tr.color}"></span>` +
      tr.kfs.map(x=>`<span class="kf" style="left:${tx(x)}px"></span>`).join('');
    tracks.appendChild(el);
  }
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
    ['viewport_tint',{color:0xdd5227}],
    ['tool_end',{id:'c7',name:'render_frame',dur_ms:380,status:'ok'}],
    ['frame_rendered',{frame:142}],
    ['tool_end',{id:'c5',name:'composite',dur_ms:31,status:'ok'}],
    ['tool_start',{id:'c8',name:'export_mp4',x:760,y:150,inputs:[{port:'in',from:'composite:out'}]}],
    ['tool_end',{id:'c8',name:'export_mp4',dur_ms:0,status:'ok'}],
    ['tool_start',{id:'c9',name:'add_clip'}],
    ['tool_end',{id:'c9',name:'add_clip',dur_ms:2,status:'ok'}],
    ['tool_start',{id:'c10',name:'search_tools',args:{q:'grain'}}],
    ['tool_end',{id:'c10',name:'search_tools',dur_ms:3,status:'ok'}],
    ['project_changed',{rev:214}],
  ];
  let i = 0;
  (function step(){
    if (i >= seq.length) return;
    const [type, body] = seq[i++];
    onEvent({ type, ...body });
    setTimeout(step, type==='tool_start' ? 500 : 900);
  })();
}
