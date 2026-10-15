/* Kimo spectator - 4-view workstation. Read-only: receives engine tool events
   via bridge.js (Tauri IPC / localhost SSE / simulated fallback). Panels whose
   data engine events do not carry yet (tracks, waves, wheels, parade, queue,
   thumbnails, scene graph) are drawn procedurally and labeled simulated. */
import { connect } from './bridge.js';
import * as THREE from './vendor/three.module.min.js';

const $ = s => document.querySelector(s);
const $$ = s => [...document.querySelectorAll(s)];
const params = new URLSearchParams(location.search);
const PINNED = params.get('view');
const T0 = performance.now();

/* deterministic rng */
function mulberry32(a){return function(){a|=0;a=a+0x6D2B79F5|0;let t=Math.imul(a^a>>>15,1|a);t=t+Math.imul(t^t>>>7,61|t)^t;return((t^t>>>14)>>>0)/4294967296;};}

/* ---------- view switching + presenter mode ---------- */
const VIEWS = ['timeline','scene','grade','export'];
const PROJ = { timeline:'dune_film.kimo', scene:'helix_study.kimo', grade:'dune_film.kimo', export:'dune_film.kimo' };
const TOOL_VIEW = { color_grade:'grade', glass_shatter:'scene', render_frame:'export', export_mp4:'export' };
let manualUntil = 0;
function setView(v, manual=false){
  if (PINNED) v = PINNED;
  if (!VIEWS.includes(v)) return;
  $$('#view-tabs button').forEach(b => b.classList.toggle('on', b.dataset.view===v));
  VIEWS.forEach(n => $('#view-'+n).classList.toggle('on', n===v));
  $('#proj-name').textContent = PROJ[v];
  if (manual) manualUntil = performance.now()+12000;
}
$$('#view-tabs button').forEach(b => b.addEventListener('click', () => setView(b.dataset.view, true)));
if (PINNED && VIEWS.includes(PINNED)) setView(PINNED);

/* ---------- procedural dune poster (labeled simulated) ---------- */
function drawDune(cv, seed=3, tone='warm'){
  const g = cv.getContext('2d'); const w = cv.width=640, h = cv.height=360;
  const rnd = mulberry32(seed*97+11);
  const sky = g.createLinearGradient(0,0,0,h*0.6);
  if (tone==='warm'){ sky.addColorStop(0,'#f4e8cf'); sky.addColorStop(1,'#ddb98a'); }
  else { sky.addColorStop(0,'#e9e2d4'); sky.addColorStop(1,'#c9b390'); }
  g.fillStyle=sky; g.fillRect(0,0,w,h*0.62);
  const sun=g.createRadialGradient(w*0.62,h*0.30,6,w*0.62,h*0.30,h*0.5);
  sun.addColorStop(0,'rgba(255,246,225,.9)'); sun.addColorStop(1,'rgba(255,246,225,0)');
  g.fillStyle=sun; g.fillRect(0,0,w,h*0.62);
  const bands=[['#d9a25f',0.46,26],['#b57c44',0.58,34],['#8a5a30',0.72,44],['#5f3c21',0.88,60]];
  bands.forEach(([col,base,amp],bi)=>{
    const f1=0.006+rnd()*0.004, f2=0.017+rnd()*0.008, p1=rnd()*6.28, p2=rnd()*6.28;
    g.beginPath(); g.moveTo(0,h);
    for(let x=0;x<=w;x+=8){ const y=h*base - amp*Math.sin(x*f1+p1) - amp*0.4*Math.sin(x*f2+p2) + bi*6; g.lineTo(x,y); }
    g.lineTo(w,h); g.closePath(); g.fillStyle=col; g.fill();
  });
  g.fillStyle='rgba(20,12,8,.85)';           // walker silhouette
  const wx=w*0.56, wy=h*0.62;
  g.beginPath(); g.arc(wx,wy-10,2.4,0,6.29); g.fill();
  g.fillRect(wx-1.6,wy-8,3.2,9);
  g.fillRect(wx-3.4,wy+1,2,6); g.fillRect(wx+1.4,wy+1,2,6);
  if (tone==='graded'){ g.fillStyle='rgba(28,84,80,.14)'; g.fillRect(0,0,w,h); }
  return cv;
}

/* ---------- preview frames ---------- */
const nleCv=$('#nle-frame'), outCv=$('#grade-out'), srcCv=$('#grade-src'), expCv=$('#export-frame');
drawDune(nleCv,3); drawDune(srcCv,5); drawDune(outCv,5,'graded'); drawDune(expCv,9);
function blit(b64){
  const img=new Image();
  img.onload=()=>{ for (const cv of [nleCv,expCv]){ const g=cv.getContext('2d');
    const s=Math.max(cv.width/img.width,cv.height/img.height), dw=img.width*s, dh=img.height*s;
    g.drawImage(img,(cv.width-dw)/2,(cv.height-dh)/2,dw,dh); } };
  img.src='data:image/png;base64,'+b64;
}

/* ---------- agent stream (real events, mirrored into all 4 views) ---------- */
const rows=[]; let callCount=0;
function fmtArgs(a){
  if(!a) return '';
  const s=Object.entries(a).slice(0,2).map(([k,v])=>`${k}: ${v}`).join(',  ');
  return s.length>34 ? s.slice(0,33)+'…' : s;
}
function pushRow(type, name, args, dur, id){
  const el=document.createElement('div');
  el.className='arow'+(type==='tool_start'?' active':'');
  const a=fmtArgs(args);
  el.innerHTML=`<span class="tick">${type==='tool_end'?'✓':''}</span>`+
    `<span class="aname">${name}${a?`<span class="aargs">(${a})</span>`:''}</span>`+
    `<span class="adur">${dur!=null?dur:''}</span>`;
  if(type==='tool_start'){ el.dataset.t0=performance.now(); kick(); }
  rows.push({el,id});
  if(rows.length>90) rows.shift();
  for(const c of $$('.agent-rows')){
    const clone=el.cloneNode(true); if(el.dataset.t0) clone.dataset.t0=el.dataset.t0;
    c.appendChild(clone);
    while(c.children.length>80) c.removeChild(c.firstChild);
    c.scrollTop=c.scrollHeight;
  }
}
function endRow(id, durMs){
  const r=[...rows].reverse().find(r=>r.id===id);
  if(!r) return;
  const idx=rows.indexOf(r);
  r.el.classList.remove('active'); delete r.el.dataset.t0;
  r.el.querySelector('.tick').textContent='✓';
  r.el.querySelector('.adur').textContent=(durMs/1000).toFixed(1)+'s';
  $$('.agent-rows').forEach(c=>{ const el=c.children[Math.min(idx,c.children.length-1)];
    if(!el) return; el.classList.remove('active'); delete el.dataset.t0;
    el.querySelector('.tick').textContent='✓'; el.querySelector('.adur').textContent=(durMs/1000).toFixed(1)+'s'; });
}

/* ---------- status pill / counts / elapsed ---------- */
let feedKind='sim';
function setPill(){
  const label = feedKind==='live' ? `live - sse - ${callCount} calls`
            : feedKind==='tauri' ? `live - engine - ${callCount} calls`
            : `sim feed - ${callCount} calls`;
  $('#live-text').textContent=label;
  $('#live-badge').classList.toggle('sim', feedKind==='sim');
  $$('.p-head .count').forEach(c=>{ c.textContent=callCount+' calls'; });
}
setInterval(()=>{ const s=Math.floor((performance.now()-T0)/1000);
  $('#elapsed').textContent=[s/3600,s/60%60,s%60].map(v=>String(Math.floor(v)).padStart(2,'0')).join(':'); },1000);

/* ---------- timeline (procedural, labeled simulated) ---------- */
const SPAN=22;
(function ruler(){
  const r=$('#tl-ruler2');
  for(let s=0;s<=SPAN;s++){
    const t=document.createElement('i');
    t.style.cssText=`position:absolute;left:${s/SPAN*100}%;bottom:0;width:1px;height:${s%5?3:6}px;background:var(--hair2)`;
    r.appendChild(t);
    if(s%5===0){ const l=document.createElement('span'); l.className='tick';
      l.style.left=(s/SPAN*100)+'%'; if(s===0) l.style.transform='none';
      l.textContent='00:'+String(s).padStart(2,'0'); r.appendChild(l); }
  }
})();
function thumb(seed,tone){ const c=document.createElement('canvas'); drawDune(c,seed,tone); return c.toDataURL('image/png'); }
function clip(lane,name,l,wd,seed,sel){
  const d=document.createElement('div'); d.className='clip'+(sel?' sel':'');
  d.style.left=l+'%'; d.style.width=wd+'%';
  d.innerHTML=`<img src="${thumb(seed)}" alt=""><span>${name}</span>`;
  $(lane).appendChild(d);
}
clip('#lane-v2','dune_aerial',0,47,11); clip('#lane-v2','sky_wide',48.6,51.4,12);
clip('#lane-v1','walk_ridge',0,28,13); clip('#lane-v1','dune_wide',30,34,14); clip('#lane-v1','dune_ridge',66,34,15,true);
[['The desert keeps time.',0,30],['Every step erased.',38,28],['Walk anyway.',70,22]].forEach(([t,l,wd])=>{
  const d=document.createElement('div'); d.className='cap'; d.style.left=l+'%'; d.style.width=wd+'%';
  d.textContent=t; $('#lane-cap').appendChild(d);
});
$$('.wave').forEach(c=>{
  const g=c.getContext('2d'); const w=c.width=Math.max(640,c.clientWidth*2||1280), h=c.height=64;
  const rnd=mulberry32(+c.dataset.seed);
  const comp=[[11,1.1,rnd()*6.28],[6,2.3,rnd()*6.28],[3,4.7,rnd()*6.28]];  // amp, freq, phase
  const env=Array.from({length:2},()=>[0.5+rnd()*0.5, 0.25+rnd()*0.3, rnd()*6.28]);
  const amp=x=>{ let a=0; for(const[A,f,p]of comp) a+=A*Math.sin(6.28*f*x/w+p);
    let e=0; for(const[A,f,p]of env) e+=A*Math.sin(6.28*f*x/w+p);
    return a*Math.max(0.35,Math.min(1,e)); };
  g.beginPath(); g.moveTo(0,h/2);
  for(let x=0;x<=w;x+=3) g.lineTo(x, h/2-Math.max(-h/2+4,Math.min(h/2-4,amp(x))));
  for(let x=w;x>=0;x-=3) g.lineTo(x, h/2+Math.max(-h/2+4,Math.min(h/2-4,amp(x))));
  g.closePath(); g.fillStyle='rgba(231,229,228,.8)'; g.fill();
});

/* ---------- playhead + timecode (advances only during activity) ---------- */
let playT=4.0, lastEvent=-1e9;
const tc=s=>`01:00:${String(Math.floor(s)).padStart(2,'0')}:${String(Math.floor(s%1*24)).padStart(2,'0')}`;
function setPlay(){ $('#tl-play').style.left=(playT/SPAN*100)+'%'; $('#nle-tc').textContent=tc(playT); }
setPlay();

/* ---------- keyframe lanes (from data-kfs) ---------- */
$$('.kf-lane').forEach(l=>{
  (l.dataset.kfs||'').split(',').filter(Boolean).forEach(p=>{
    const d=document.createElement('span'); d.className='kf'+(p===l.dataset.hot?' hot':'');
    d.style.left=p+'%'; l.appendChild(d);
  });
});

/* ---------- 3D scene: helix plates (three.js, labeled simulated) ---------- */
(function scene(){
  const canvas=$('#gl');
  const renderer=new THREE.WebGLRenderer({canvas,antialias:true});
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));
  const sc=new THREE.Scene(); sc.background=new THREE.Color(0x17151a);
  sc.fog=new THREE.Fog(0x17151a,8,18);
  const cam=new THREE.PerspectiveCamera(35,1,0.1,100);
  cam.position.set(4.2,3.0,5.2); cam.lookAt(0,1.9,0);
  const plateG=new THREE.BoxGeometry(1.9,0.05,0.5);
  const plateM=new THREE.MeshStandardMaterial({color:0xd9d2c4,roughness:0.42,metalness:0.25});
  const helix=new THREE.Group();
  for(let i=0;i<56;i++){ const m=new THREE.Mesh(plateG,plateM);
    const a=i*0.22; m.position.set(Math.cos(a)*0.55, i*0.075, Math.sin(a)*0.55); m.rotation.y=-a;
    helix.add(m); }
  sc.add(helix);
  const floor=new THREE.Mesh(new THREE.CircleGeometry(6,48),
    new THREE.MeshStandardMaterial({color:0x1c1a20,roughness:0.9}));
  floor.rotation.x=-Math.PI/2; sc.add(floor);
  const key=new THREE.DirectionalLight(0xfff0dd,2.1); key.position.set(3,4,2); sc.add(key);
  const rim=new THREE.DirectionalLight(0xbcd2ff,0.9); window.__rim=rim; rim.position.set(-4,2,-3); sc.add(rim);
  sc.add(new THREE.AmbientLight(0xf4f0e8,0.35));
  let dirty=true, raf=0, last=0;
  const spinUntil = matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : performance.now()+4000;
  function frame(t){ raf=0;
    const w=canvas.clientWidth, h=canvas.clientHeight;
    if(w&&h&&(canvas.width!==w*renderer.getPixelRatio()||canvas.height!==h*renderer.getPixelRatio())){
      renderer.setSize(w,h,false); cam.aspect=w/h; cam.updateProjectionMatrix(); dirty=true; }
    const spinning=t<spinUntil;
    if((dirty||spinning)&&t-last>=33){
      if(spinning){ const a=t/1000*0.5; cam.position.set(Math.sin(a)*5.4,2.8,Math.cos(a)*5.4); cam.lookAt(0,1.1,0); }
      renderer.render(sc,cam); dirty=false; last=t; }
    if(dirty||spinning) raf=requestAnimationFrame(frame);
  }
  const mark=()=>{ dirty=true; if(!raf) raf=requestAnimationFrame(frame); };
  new ResizeObserver(mark).observe(canvas); mark();
  window.__tintViewport = c => { rim.color.set(c); rim.intensity=1.6; mark(); };
})();

/* curve editor time labels (aligned with kf lanes) */
(function curveLabels(){
  const wrap=$('#curve-wrap'); if(!wrap) return;
  const row=document.createElement('div'); row.id='curve-labels';
  for(let s=0;s<=6;s++){ const l=document.createElement('span'); l.style.left=(s/6*100)+'%'; l.textContent=s+'s'; row.appendChild(l); }
  wrap.after(row);
})();

/* ---------- grade wheels + parade (simulated) ---------- */
(function wheels(){
  const defs=[['Lift',['-0.02','0.00','+0.03'],0,false],['Gamma',['+0.01','0.00','-0.01'],0,false],['Gain',['+0.06','+0.04','+0.02'],0.9,true]];
  const hues=['#e86a6a','#e8c86a','#7fc97f','#6ac8c8','#6a9ae8','#b07fe8'];
  $('#wheels').innerHTML=defs.map(([name,vals,off,hot])=>{
    const arcs=hues.map((hcol,i)=>{ const a0=(i*60-88)*Math.PI/180, a1=((i+1)*60-92)*Math.PI/180;
      const x0=43+34*Math.cos(a0),y0=43+34*Math.sin(a0),x1=43+34*Math.cos(a1),y1=43+34*Math.sin(a1);
      return `<path d="M ${x0} ${y0} A 34 34 0 0 1 ${x1} ${y1}" fill="none" stroke="${hcol}" stroke-opacity=".28" stroke-width="5"/>`; }).join('');
    const da=off*Math.PI*2-Math.PI/2, dx=43+20*Math.cos(da), dy=43+20*Math.sin(da);
    return `<div class="wheel"><svg width="86" height="86" viewBox="0 0 86 86">
      <circle cx="43" cy="43" r="34" fill="none" stroke="rgba(231,229,228,.16)" stroke-width="5"/>${arcs}
      <line x1="43" y1="6" x2="43" y2="80" stroke="rgba(231,229,228,.1)"/><line x1="6" y1="43" x2="80" y2="43" stroke="rgba(231,229,228,.1)"/>
      <circle cx="${dx}" cy="${dy}" r="5" fill="${hot?'#ff5a23':'#e7e5e4'}"/></svg>
      <div class="wname">${name}</div>
      <div class="wvals">${vals.map((v,i)=>i===0&&hot?`<span class="hot">${v}</span>`:v).join('&nbsp;&nbsp;')}</div></div>`;
  }).join('');
})();
(function parade(){
  const c=$('#parade'), g=c.getContext('2d');
  const w=c.width=Math.max(560,c.clientWidth*2||900), h=c.height=300;
  const levels=[0,256,512,768,1023];
  g.font='20px Martian Mono, monospace'; g.fillStyle='#6e665e'; g.strokeStyle='rgba(231,229,228,.08)';
  levels.forEach(v=>{ const y=h-8-(v/1023)*(h-20);
    g.beginPath(); g.moveTo(56,y); g.lineTo(w,y); g.stroke(); g.fillText(String(v),4,y+6); });
  const rnd=mulberry32(41);
  [['rgba(232,106,106,',0.72],['rgba(127,201,127,',0.55],['rgba(138,127,232,',0.65]].forEach(([col,amp],ci)=>{
    const x0=64+ci*(w-80)/3, cw=(w-80)/3-12;
    const bumps=Array.from({length:7},()=>[rnd()*cw,14+rnd()*30,0.3+rnd()*0.7]);
    g.beginPath(); g.moveTo(x0,h-8);
    for(let x=0;x<=cw;x+=3){ let a=0.18; for(const[bx,bw,ba]of bumps){const d=(x-bx)/bw; a+=ba*Math.exp(-d*d);}
      g.lineTo(x0+x, h-8-Math.min(a,1)*amp*(h-24)); }
    g.lineTo(x0+cw,h-8); g.closePath();
    g.fillStyle=col+'0.5)'; g.fill(); g.strokeStyle=col+'0.95)'; g.lineWidth=1.4; g.stroke();
  });
})();
(function thumbs3(){
  const box=$('#grade-thumbs'); if(!box) return;
  [21,22,23].forEach((s,i)=>{ const c=document.createElement('canvas'); c.width=104; c.height=68;
    drawDune(c,s,i===1?'graded':'warm'); c.className='thumb'+(i===1?' sel':''); box.appendChild(c); });
})();

/* ---------- export: filmstrip + progress (simulated) ---------- */
(function strip(){
  const box=$('#filmstrip'); if(!box) return;
  for(let i=0;i<8;i++){ const c=document.createElement('canvas'); drawDune(c,31+i, i===4?'graded':'warm');
    if(i===4) c.className='sel'; box.appendChild(c); }
})();
let expActive=false, expPct=64;
function renderExp(){
  $('#exp-prog').style.width=expPct+'%';
  $('#exp-frame-no').textContent=Math.round(expPct/100*2160);
  $('#exp-pct').textContent=Math.floor(expPct)+'%';
  $('#exp-fps').textContent=(41+Math.round(Math.sin(performance.now()/700)*2+2))+' fps';
  $('#exp-eta').textContent='eta 00:'+String(Math.max(2,Math.round((100-expPct)*0.28))).padStart(2,'0');
  const pill=$('#queue tbody tr .st-pill.run');
  if(pill) pill.innerHTML=`<span class="dot"></span>rendering ${Math.floor(expPct)}%`;
}
renderExp();

/* ---------- shared rAF: playhead, active durations, export progress ---------- */
let rafOn=false, lastTick=0;
function kick(){ if(!rafOn){ rafOn=true; lastTick=performance.now(); requestAnimationFrame(loop); } }
function loop(t){
  const dt=(t-lastTick)/1000; lastTick=t;
  const active=t-lastEvent<2500;
  if(active&&playT<21.5){ playT=Math.min(21.5,playT+dt*0.6); setPlay(); }
  for(const c of $$('.agent-rows')) for(const el of c.querySelectorAll('.arow.active')){
    if(el.dataset.t0) el.querySelector('.adur').textContent=((t-el.dataset.t0)/1000).toFixed(1)+'s';
  }
  if(expActive){ expPct=Math.min(96,expPct+dt*1.1); renderExp(); }
  const anyActive = $$('.agent-rows .arow.active').length>0;
  if(active||expActive||anyActive) requestAnimationFrame(loop); else rafOn=false;
}

/* ---------- events ---------- */
function onEvent(ev){
  lastEvent=performance.now(); kick();
  if(ev.type==='tool_start'){
    callCount++; setPill();
    pushRow('tool_start',ev.name,ev.args,null,ev.id);
    const v=TOOL_VIEW[ev.name]||'timeline';
    if(!PINNED&&performance.now()>manualUntil) setView(v);
    if(ev.name==='export_mp4'){ expActive=true; expPct=Math.min(expPct,64); }
    $('#nle-chip').textContent=`${ev.name} - clip_04`;
  } else if(ev.type==='tool_end'){
    endRow(ev.id,ev.dur_ms||0);
    if(ev.name==='export_mp4') expActive=false;
  } else if(ev.type==='frame_rendered'){ if(ev.png_b64) blit(ev.png_b64);
  } else if(ev.type==='viewport_tint'){ window.__tintViewport&&window.__tintViewport(ev.color); }
}

/* ---------- transport ---------- */
connect(onEvent, st=>{ feedKind=(st==='live')?'live':'sim'; setPill(); }, ()=>{ feedKind='sim'; setPill(); runSim(); });

/* procedural stand-in PNG for the sim feed (exercises the blit path) */
function demoFramePng(){ const c=document.createElement('canvas'); drawDune(c,77); return c.toDataURL('image/png').split(',')[1]; }
function runSim(){
  const seq=[
    ['tool_start',{id:'c1',name:'load_media',args:{file:'dune_aerial.mov'}}],
    ['tool_end',{id:'c1',dur_ms:400}],
    ['tool_start',{id:'c2',name:'import_media',args:{file:'walk_ridge.mov'}}],
    ['tool_end',{id:'c2',dur_ms:400}],
    ['tool_start',{id:'c3',name:'color_grade',args:{clip:'clip_04',warmth:'+18'}}],
    ['tool_end',{id:'c3',dur_ms:800}],
    ['tool_start',{id:'c4',name:'glass_shatter',args:{pieces:512,seed:7}}],
    ['tool_end',{id:'c4',dur_ms:300}],
    ['tool_start',{id:'c5',name:'composite',args:{mode:'over'}}],
    ['tool_start',{id:'c6',name:'render_frame',args:{f:142}}],
    ['viewport_tint',{color:0xff5a23}],
    ['tool_end',{id:'c6',dur_ms:380}],
    ['frame_rendered',{frame:142,png_b64:demoFramePng()}],
    ['tool_end',{id:'c5',dur_ms:900}],
    ['tool_start',{id:'c7',name:'export_mp4',args:{preset:'web_1080',crf:18}}],
    ['tool_end',{id:'c7',dur_ms:38400}],
    ['tool_start',{id:'c8',name:'add_clip',args:{clip:'sky_wide'}}],
    ['tool_end',{id:'c8',dur_ms:100}],
    ['project_changed',{rev:214}],
  ];
  let i=0;
  (function step(){ if(i>=seq.length) return;
    const[type,body]=seq[i++]; onEvent({type,...body});
    setTimeout(step, type==='tool_start'?500:900); })();
}
