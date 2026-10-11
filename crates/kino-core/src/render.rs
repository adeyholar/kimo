use crate::effects::{self, Frame};
use crate::model::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// Decoded frames for media clips, keyed by clip id. Frames are RGBA8 at project size, project fps,
/// Constant speed is applied during decode; ramps use source-time indices.
/// A decoded window of consecutive frames, contiguous in memory. `first` is the global frame index of buf[0].
pub struct Frames { pub still:bool,pub working_linear:bool, pub quantized: std::sync::OnceLock<Vec<u8>>, pub linear_buf: Option<Vec<f32>>, pub buf: Vec<u8>, pub fs: usize, pub first: usize }
impl Frames {
    pub fn get_float(&self,g:usize)->Option<&[f32]>{let b=self.linear_buf.as_ref()?;let n=self.len();if n==0{return None}let i=g.saturating_sub(self.first).min(n-1);Some(&b[i*self.fs..(i+1)*self.fs])}
    pub fn len(&self) -> usize { if self.fs == 0 { 0 } else { self.linear_buf.as_ref().map(|b|b.len()).unwrap_or(self.buf.len()) / self.fs } }
    pub fn covers(&self, a: usize, b: usize) -> bool { self.len() > 0 && ((a >= self.first && b < self.first + self.len()) || (self.still && self.len() == 1 && self.first == 0)) }
    /// Global-index lookup, clamped into the window.
    pub fn get(&self, g: usize) -> Option<&[u8]> { let n = self.len(); if n == 0 { return None; } let i = g.saturating_sub(self.first).min(n - 1); let buf=if let Some(f)=&self.linear_buf{self.quantized.get_or_init(||f.iter().map(|v|(v.clamp(0.,1.)*255.+0.5) as u8).collect())}else{&self.buf};Some(&buf[i * self.fs..(i + 1) * self.fs]) }
}
impl std::ops::Index<usize> for Frames { type Output = [u8]; fn index(&self, i: usize) -> &[u8] { self.get(self.first+i).expect("no decoded frame") } }
pub struct Cache { frames: HashMap<String, (String, std::sync::Arc<Frames>)>, pub size: (u32, u32, f64) }
impl Cache { pub fn new() -> Self { Cache { frames: HashMap::new(), size: (0, 0, 0.0) } } pub fn frames_of(&self, id: &str) -> Option<&Frames> { self.frames.get(id).map(|x| x.1.as_ref()) } }

pub fn probe(path: &str) -> Result<(u32, u32, f64, f64), String> {
    if [".tif",".tiff"].iter().any(|e|path.to_lowercase().ends_with(e)){let(w,h)=crate::color::interchange::tiff_size(path)?;return Ok((w,h,0.,25.))}
    let out = Command::new("ffprobe").args(["-v","error","-select_streams","v:0","-show_entries","stream=width,height,r_frame_rate,duration","-of","csv=p=0",path])
        .output().map_err(|e| format!("ffprobe: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout); let s = s.trim();
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() < 3 {
        let o = Command::new("ffprobe").args(["-v","error","-show_entries","format=duration","-of","csv=p=0",path]).output().map_err(|e| e.to_string())?;
        let d: f64 = String::from_utf8_lossy(&o.stdout).trim().parse().map_err(|_| format!("cannot probe {path}"))?;
        return Ok((0, 0, d, 0.0));
    }
    let fr: Vec<f64> = parts[2].split('/').filter_map(|x| x.parse().ok()).collect();
    let fps = if fr.len() == 2 && fr[1] > 0.0 { fr[0]/fr[1] } else { 30.0 };
    Ok((parts[0].parse().unwrap_or(0), parts[1].parse().unwrap_or(0), parts.get(3).and_then(|d| d.parse().ok()).unwrap_or(0.0), fps))
}

struct DecodeProcess(std::process::Child);
impl Drop for DecodeProcess{fn drop(&mut self){let _=self.0.kill();let _=self.0.wait();}}
const WINDOW: usize = 24;

/// Decode `n` frames starting at global clip frame `a` (windowed: memory stays bounded for any clip length).
fn decode(p: &Project, c: &Clip, path: &str, a: usize, n: usize) -> Result<Frames, String> {
    let (w, h) = (p.width, p.height);
    if p.color_pipeline.enabled&&[".tif",".tiff"].iter().any(|e|path.to_lowercase().ends_with(e)){
      let mut src=crate::color::interchange::tiff_frame(path,decode_cache_limit())?;
      let fit=c.props.get("fit").map(|q|q.at(0.)).unwrap_or(0.);
      crate::color::pipeline::input(p,c,&mut src)?;
      let d=crate::color::interchange::fit(&src,w as usize,h as usize,fit)?;
      return Ok(Frames{still:true,working_linear:true,quantized:Default::default(),linear_buf:Some(d),buf:vec![],fs:w as usize*h as usize*4,first:0})
    } let is_img0 = { let l = path.to_lowercase(); ["png","jpg","jpeg","webp","bmp","exr","tif","tiff","pfm"].iter().any(|e| l.ends_with(e)) }; let (a, n) = if is_img0 { (0, 1) } else { (a, n) };
    let (in_pt, speed, span) = if c.speed_curve.is_some() { (c.in_point, 1.0, (a+n) as f64 / p.fps) } else { (c.in_point, c.speed, (a+n) as f64 / p.fps * c.speed) };
    let fit = c.props.get("fit").map(|q| q.at(0.0)).unwrap_or(0.0);
    let fitf = if fit >= 1.5 { format!("scale={w}:{h}") } else if fit >= 0.5 { format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}") } else { format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black@0") };
    if p.color_pipeline.enabled&&path.to_lowercase().ends_with(".exr"){crate::color::interchange::validate_exr_source(path)?;}
    let float_source=p.color_pipeline.enabled&&[".exr",".pfm"].iter().any(|ext|path.to_lowercase().ends_with(ext));
    let native_fit=float_source||(p.color_pipeline.enabled&&p.color_pipeline.linear_resample);
    let pix=if float_source{"gbrapf32le"}else if p.color_pipeline.enabled{"rgba64le"}else{"rgba"};
    let vf = format!("setpts=PTS/{},fps={},trim=start_frame={a}:end_frame={},setpts=PTS-STARTPTS,{},format={pix}",speed.max(0.01),p.fps,a+n,fitf);
    let is_img = ["png","jpg","jpeg","webp","bmp","exr","tif","tiff","pfm"].iter().any(|e| path.to_lowercase().ends_with(e));
    let loop_args: Vec<&str> = if is_img { vec!["-loop", "1"] } else { vec![] };
    let fs = (w * h * 4) as usize;


    let (dw,dh)=if native_fit{let(q,r,_,_)=probe(path)?;(q as usize,r as usize)}else{(w as usize,h as usize)};
    let raw_fs=dw.checked_mul(dh).and_then(|v|v.checked_mul(4)).ok_or("source dimensions overflow")?;
    let estimate=raw_fs.max(fs).checked_mul(if p.color_pipeline.enabled{8}else{1}).ok_or("decode allocation overflow")?.checked_mul(n.max(1)).ok_or("decode allocation overflow")?;
    if estimate>decode_cache_limit(){return Err("decode request exceeds bounded cache; reduce size/duration or use windowed timing".into())}
    crate::guard::reserve(estimate);
    let exr_args:Vec<&str>=if path.to_lowercase().ends_with(".exr"){vec!["-apply_trc","linear"]}else{vec![]};
    let vf=if native_fit{format!("setpts=PTS/{},fps={},trim=start_frame={a}:end_frame={},setpts=PTS-STARTPTS,format={pix}",speed.max(0.01),p.fps,a+n)}else{vf};
    let mut command=Command::new("ffmpeg");command.args(["-v","error"]).args(&loop_args).args(&exr_args).args(["-ss",&format!("{in_pt}"),"-t",&format!("{span}"),"-i",path,"-vf",&vf,"-frames:v",&n.max(1).to_string(),"-f","rawvideo","-pix_fmt",pix,"-"]);
    let mut buf=Vec::new();
    let linear_buf=if native_fit{
        let bytes_per_frame=raw_fs.checked_mul(if float_source{4}else{2}).ok_or("native frame size overflow")?;
        let converter=if float_source{None}else{Some(crate::color::pipeline::Input16::new(p,c)?)};
        let peak=bytes_per_frame.checked_mul(2).and_then(|v|v.checked_add(raw_fs.checked_mul(4)?)).and_then(|v|v.checked_add(fs.checked_mul(4)?.checked_mul(n.max(1).checked_add(1)?)?)).ok_or("native streaming allocation overflow")?;
        crate::guard::reserve(peak);
        let mut stream=crate::color::process::DecodeStream::new(&mut command,bytes_per_frame,n.max(1),crate::color::process::decode_deadline(n.max(1)))?;
        let mut values=Vec::with_capacity(fs*n.max(1));
        while let Some(frame)=stream.next()?{
         let d=if float_source{crate::color::interchange::unpack(&frame,dw,dh)}else{converter.as_ref().ok_or("missing integer input converter")?.decode(&frame)};
         let fitted=d.and_then(|d|{let mut source=Frame{w:dw,h:dh,d};if float_source{crate::color::pipeline::input(p,c,&mut source)?;}crate::color::interchange::fit(&source,w as usize,h as usize,fit)});values.extend(fitted?);
        }
        stream.finish()?;Some(values)
    }else{
        let bytes_per_frame=fs.checked_mul(if p.color_pipeline.enabled{2}else{1}).ok_or("decode frame size overflow")?;
        let max=bytes_per_frame.checked_mul(n.max(1)).ok_or("decode output size overflow")?;
        buf=crate::color::process::decode_capture(&mut command,max,crate::color::process::decode_deadline(n.max(1)))?;
        if buf.len()%bytes_per_frame!=0{return Err("truncated project-sized decode output".into())}
        if p.color_pipeline.enabled{let values=buf.chunks_exact(2).map(|v|u16::from_le_bytes([v[0],v[1]]) as f32/65535.).collect();buf=Vec::new();Some(values)}else{None}
    };
    let k = buf.len() / fs; buf.truncate(k * fs);
    Ok(Frames { still:is_img,working_linear:native_fit,quantized:Default::default(),linear_buf, buf, fs, first: a })
}

fn media_index(c:&Clip,lt:f64,fps:f64)->usize{if c.speed_curve.is_some(){return c.source_index(lt,fps)}(lt.max(0.)*fps).floor() as usize}
fn media_total(c:&Clip,fps:f64)->usize{let span=if c.speed_curve.is_some(){c.source_time(c.duration)}else{c.duration};((span*fps).ceil() as usize).max(1)}
fn decode_cache_limit()->usize{(crate::guard::limits().mem_bytes/3).min(256*1024*1024).max(1024*1024)}

pub fn prepare(p: &Project, cache: &mut Cache) -> Result<(), String> { prepare_at(p, cache, None) }

/// Decode media clips (only those active at `t` when given). Re-decodes when clip timing/speed/project size changed.
pub fn prepare_at(p: &Project, cache: &mut Cache, t: Option<f64>) -> Result<(), String> { match t { Some(t) => prepare_range(p, cache, t, t), None => prepare_range(p, cache, f64::MIN, f64::MAX) } }

/// Decode clips overlapping [t0, t1] and evict decoded clips outside it (bounds memory).
pub fn prepare_range(p: &Project, cache: &mut Cache, t0: f64, t1: f64) -> Result<(), String> {prepare_window(p,cache,t0,t1,WINDOW)}
/// Export-only bounded lookahead amortizes stable prefix decode without changing fps phase.
pub fn prepare_export_range(p:&Project,cache:&mut Cache,t0:f64,t1:f64)->Result<(),String>{prepare_window(p,cache,t0,t1,256)}
fn prepare_window(p: &Project, cache: &mut Cache, t0: f64, t1: f64, window:usize) -> Result<(), String> {
    if !t0.is_finite()||!t1.is_finite()||t1<t0{return Err("decode range requires finite ordered times".into())}
    if cache.size != (p.width, p.height, p.fps) { cache.frames.clear(); cache.size = (p.width, p.height, p.fps); }
    let all = t0 == f64::MIN;
    let flat = flat_clips(&p.tracks, 0.0);
    let active: std::collections::HashSet<String> = flat.iter().filter(|c| all || (c.start <= t1 && c.start + c.duration > t0)).map(|c| c.id.clone()).collect();
    if !all { cache.frames.retain(|k, _| active.contains(k)); }
    let media_count=flat.iter().filter(|c|active.contains(&c.id)&&matches!(c.source,Source::Media{..})&&c.props.get("audio_only").map(|q|q.at(0.)).unwrap_or(0.)<0.5).count().max(1);
    if !all{let budget=decode_cache_limit()/media_count;cache.frames.retain(|_,(_,fr)|fr.buf.len()+fr.linear_buf.as_ref().map(|b|b.len()*5).unwrap_or(0)<=budget);}
    for c in &flat {
        let Source::Media { asset } = &c.source else { continue };
        if !active.contains(&c.id) || c.props.get("audio_only").map(|q| q.at(0.0)).unwrap_or(0.0) >= 0.5 { continue; }
        let total = media_total(c,p.fps);
        let (ta, tb) = match c.time_mode { TimeMode::Normal => (t0, t1), TimeMode::Reverse => (c.start + c.duration - (t1 - c.start), c.start + c.duration - (t0 - c.start)), TimeMode::Freeze { at } => (c.start + at, c.start + at) };
        let (t0, t1) = (ta, tb);
        let (ia, mut ib) = if all { (0, total - 1) } else if c.speed_curve.is_some(){(media_index(c,(t0-c.start).min(c.duration),p.fps).min(total-1),media_index(c,(t1-c.start).min(c.duration),p.fps).min(total-1))}else { ((((t0 - c.start).max(0.0) * p.fps).floor() as usize).min(total - 1), (((t1 - c.start).max(0.0) * p.fps).ceil() as usize).min(total - 1)) };
        // Instant normal playback samples floor(index), except flow needs the next frame.
        if !all&&matches!(c.time_mode,TimeMode::Normal)&&t0==t1{let flow=c.props.get("flow_blur").map(|v|v.at((t0-c.start).max(0.))>0.).unwrap_or(false);ib=if flow{(ia+1).min(total-1)}else{ia};}
        let a = p.assets.get(asset).ok_or(format!("asset {asset} missing"))?;
        let stat=std::fs::metadata(&a.path).map_err(|e|format!("media stat: {e}"))?;
        let sig = format!("{asset}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}|{:?}", c.in_point, c.duration, c.speed, c.speed_curve.as_ref().map(|p| serde_json::to_string(p).unwrap_or_default()), p.color_pipeline,c.props.get("input_gamut"),c.props.get("input_transfer"),c.props.get("fit"),a.path,stat.len(),stat.modified().ok());
        if let Some((s, fr)) = cache.frames.get(&c.id) { if *s == sig && fr.covers(ia, ib) { continue; } }
        let shared=cache.frames.values().find(|(s,fr)|*s==sig&&fr.covers(ia,ib)).map(|(_,fr)|fr.clone());
        if let Some(fr)=shared{cache.frames.insert(c.id.clone(),(sig,fr));continue}
        if a.width == 0 { cache.frames.insert(c.id.clone(), (sig, std::sync::Arc::new(Frames { still:false,working_linear:false,quantized:Default::default(),linear_buf: None, buf: vec![], fs: 0, first: 0 }))); continue; }
        // Ramps cache source-frame windows; constant-speed clips cache output-frame windows.
        let (start, count) = if all { (0, total) } else { (ia, (ib - ia + 1).max(window).min(total - ia)) };
        let mut count=count;
        if !all{let native=p.color_pipeline.enabled&&p.color_pipeline.linear_resample;let pixels=if native{let(dw,dh,_,_)=probe(&a.path)?;(dw as usize*dh as usize).max(p.width as usize*p.height as usize)}else{p.width as usize*p.height as usize};let per=pixels*4*if p.color_pipeline.enabled{8}else{1};count=count.min((decode_cache_limit()/media_count/per.max(1)).max(1));if count<ib-ia+1{return Err("active decode range exceeds cache budget".into())}}
        cache.frames.remove(&c.id);
        let fr = decode(p, c, &a.path, start, count)?;
        if c.speed_curve.is_some()&&!fr.covers(ia,ib){return Err(format!("speed ramp overrun for clip '{}': requested source frames {ia}..={ib}, decoded {} frame(s) starting at {}; shorten the ramp/clip or choose media with enough source duration",c.id,fr.len(),fr.first))}
        let bytes=|fr:&Frames|fr.buf.len()+fr.linear_buf.as_ref().map(|b|b.len()*5).unwrap_or(0);
        let mut seen=std::collections::HashSet::new();
        if cache.frames.values().filter(|(_,f)|seen.insert(std::sync::Arc::as_ptr(f))).map(|(_,f)|bytes(f)).sum::<usize>().saturating_add(bytes(&fr))>decode_cache_limit(){return Err("active media cache exceeds memory cap; reduce resolution or active clips".into())}
        cache.frames.insert(c.id.clone(), (sig, std::sync::Arc::new(fr)));
    }
    Ok(())
}

/// All clips including those inside precomps, with starts shifted onto the project clock (precomps nest recursively).
pub fn flat_clips(tracks: &[Track], off: f64) -> Vec<Clip> { let mut v = vec![]; for tr in tracks { for c in &tr.clips { let mut cc = c.clone(); cc.start += off; if let Source::Precomp { tracks: inner } = &c.source { let sub = flat_clips(inner, cc.start); cc.source = Source::Adjustment; v.push(cc); v.extend(sub); } else { v.push(cc); } } } v }

fn source_frame(p: &Project, c: &Clip, lt_in: f64, cache: &Cache) -> Frame {
    let lt = c.display_time(lt_in);
    let (w, h) = (p.width as usize, p.height as usize);
    let mut f = Frame::new(w, h);
    let mut source_linear=false;
    match &c.source {
        Source::Solid { color } => for i in 0..w*h { f.d[i*4]=color[0] as f32; f.d[i*4+1]=color[1] as f32; f.d[i*4+2]=color[2] as f32; f.d[i*4+3]=1.0; },
        Source::Gradient { c1, c2 } => for y in 0..h { let k = y as f64 / h as f64; for x in 0..w { let i=(y*w+x)*4; for j in 0..3 { f.d[i+j] = (c1[j]*(1.0-k)+c2[j]*k) as f32; } f.d[i+3]=1.0; } },
        Source::Adjustment => {}
        Source::Precomp { tracks } => { let mut sub = Project { occluders: p.occluders.clone(), color_pipeline: p.color_pipeline.clone(), tracks: tracks.clone(), assets: p.assets.clone(), camera: p.camera.clone(), markers: vec![], name: String::new(), width: p.width, height: p.height, fps: p.fps, next_id: 0 }; sub.name = c.id.clone(); match render_frame_opts(&sub, lt, cache, true) {Ok(fr)=>f=fr,Err(_)=>f.d.fill(f32::NAN)} }
        Source::Shape { shape, width, height, fill, stroke_width, stroke_color, sides, corner_radius } => {
            let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0); let (hw, hh) = (width / 2.0, height / 2.0);
            // signed distance (negative inside)
            let sd = |x: f64, y: f64| -> f64 { let (px, py) = (x - cx, y - cy); match shape.as_str() {
                "ellipse" => { let k = ((px / hw).powi(2) + (py / hh).powi(2)).sqrt(); (k - 1.0) * hw.min(hh) }
                "polygon" => { let n = (*sides).max(3) as f64; let r = hw.min(hh); let a = py.atan2(px) + std::f64::consts::FRAC_PI_2; let seg = std::f64::consts::TAU / n; let aa = (a.rem_euclid(seg)) - seg / 2.0; (px * px + py * py).sqrt() * aa.cos() - r * (seg / 2.0).cos() }
                "line" => (py.abs() - hh.max(1.0)).max(px.abs() - hw),
                _ => { let (qx, qy) = (px.abs() - hw + corner_radius, py.abs() - hh + corner_radius); (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - corner_radius } } };
            for y in 0..h { for x in 0..w { let d = sd(x as f64 + 0.5, y as f64 + 0.5); let inside = (0.5 - d).clamp(0.0, 1.0); if inside <= 0.0 && *stroke_width <= 0.0 { continue; }
                let sk = if *stroke_width > 0.0 { (0.5 - (d.abs() - stroke_width / 2.0)).clamp(0.0, 1.0) } else { 0.0 }; let fl = if fill[0] < 0.0 { 0.0 } else if *stroke_width > 0.0 { (0.5 - (d + stroke_width / 2.0)).clamp(0.0, 1.0) } else { inside };
                let i = (y * w + x) * 4; let a = (fl + sk * (1.0 - fl)).clamp(0.0, 1.0); if a <= 0.0 { continue; }
                for j in 0..3 { f.d[i + j] = ((fill[j] * fl + stroke_color[j] * sk * (1.0 - fl)) / a) as f32; } f.d[i + 3] = a as f32; } }
        }
        Source::Emitter(e) => crate::particles::render(e, &mut f, lt),
        Source::SpeedLines(s) => crate::speedlines::render(s, &mut f, lt),
        Source::GlitchConfetti(s) => crate::confetti::render(s, &mut f, lt),
        Source::Scene3D(e) => crate::scene3d::render(e, &mut f, lt, &crate::three_d::Camera::from_props(&p.camera, c.start + lt)),
        Source::ShardBurst(e) => { if let Some(tid) = e.plane.as_ref().and_then(|pl| pl.occlusion_target.as_ref()) { let tc = flat_clips(&p.tracks, 0.0).into_iter().find(|x| &x.id == tid); if let (Some(tc), Some(sp)) = (tc, p.occluders.get(tid)) { if let Some(vis) = crate::planar::adaptive_matte(&tc, sp, p.fps, c.start + lt, f.w, f.h) { let occ: Vec<f32> = vis.iter().map(|v| 1.0 - v).collect(); crate::shards::EXT_OCC.with(|cell| *cell.borrow_mut() = Some(std::sync::Arc::new(occ))); } } } crate::shards::WORK_LINEAR.store(p.color_pipeline.enabled && p.color_pipeline.linear_resample, std::sync::atomic::Ordering::Relaxed); crate::shards::render(e, &mut f, lt, &crate::three_d::Camera::from_props(&p.camera, c.start + lt), p.fps, c.duration) }
        Source::Particles { kind, count, color, size, seed } => {
            let rnd = |i: u64, k: u64| { let mut x = (seed ^ (i * 0x9E3779B97F4A7C15) ^ (k * 0xBF58476D1CE4E5B9)).wrapping_mul(0x94D049BB133111EB); x ^= x >> 29; (x >> 11) as f64 / (1u64 << 53) as f64 };
            let (wf, hf) = (w as f64, h as f64);
            for i in 0..*count as u64 {
                let (r0, r1, r2, r3) = (rnd(i, 1), rnd(i, 2), rnd(i, 3), rnd(i, 4));
                let life = 1.0 + r3 * 2.0; let ph = (lt / life + r0).fract();
                let (x, y, s, a) = match kind.as_str() {
                    "snow" => (((r1 + 0.05 * (lt * 2.0 + r2 * 6.0).sin()) * wf), ph * hf, size * (0.5 + r2), 0.8),
                    "rain" => (r1 * wf, ph * hf * 1.2 - 0.1 * hf, size * 0.4, 0.5),
                    "embers" | "sparks" => { let sp = if kind == "sparks" { 2.5 } else { 1.0 }; (r1 * wf + (r2 - 0.5) * wf * 0.3 * ph * sp, hf - ph * hf * (0.5 + r2 * 0.6) * sp, size * (1.0 - ph) * (0.5 + r3), (1.0 - ph).powf(1.5)) }
                    _ => ((r1 + 0.03 * (lt + r2 * 9.0).sin()) * wf, ((r2 + 0.02 * lt * (0.5 + r3)).fract()) * hf, size * (0.5 + r3), 0.35 * (0.5 + 0.5 * (lt * 2.0 + r0 * 6.28).sin())),
                };
                let rad = s.max(0.5); let (x0, x1, y0, y1) = ((x - rad * 2.5).max(0.0) as usize, ((x + rad * 2.5) as usize).min(w - 1), (y - rad * 2.5).max(0.0) as usize, ((y + rad * 2.5) as usize).min(h - 1));
                for yy in y0..=y1 { for xx in x0..=x1 { let d = (((xx as f64 - x).powi(2) + (yy as f64 - y).powi(2)).sqrt()) / rad; let k = (a * (1.0 - d / 2.5).max(0.0).powi(2)) as f32; if k <= 0.0 { continue; }
                    let o = (yy * w + xx) * 4; for j in 0..3 { f.d[o + j] = (f.d[o + j] * f.d[o + 3] * (1.0 - k) + color[j] as f32 * k) / (f.d[o + 3] + k * (1.0 - f.d[o + 3])).max(1e-6); } f.d[o + 3] += k * (1.0 - f.d[o + 3]); } }
            }
        }
        Source::Text { text, font, size, color, letter_spacing, stroke_width, stroke_color, shadow, align } => {
            let text = &dyn_text(text, c.start + lt, p.fps);
            let _ = crate::text::render_text(&mut f, text, font, *size, *color, *letter_spacing, *stroke_width, *stroke_color, *shadow, align, prop(c, "reveal", lt), c.text_fx.as_ref(), lt);
        }
        Source::Media { .. } => if let Some(fr) = cache.frames_of(&c.id) {
            source_linear=fr.working_linear;
            let idx = media_index(c,lt,p.fps);
            if let Some(b)=fr.get_float(idx){f.d.copy_from_slice(b)}else if let Some(b) = fr.get(idx) { for (i, v) in b.iter().enumerate() { f.d[i] = *v as f32 / 255.0; } }
            let fb = prop(c, "flow_blur", lt) as f32;
            if fb > 0.0 { let nxt=if fr.len()>0 {Some(idx+1)}else{None};
                if let Some(next_idx) = nxt { let mut g2 = Frame::new(w, h);
                    if let Some(b)=fr.get_float(next_idx){g2.d.copy_from_slice(b)}else if let Some(nb)=fr.get(next_idx){for (i,v) in nb.iter().enumerate(){g2.d[i]=*v as f32/255.;}}
                    if p.color_pipeline.enabled && !source_linear {let a=crate::color::pipeline::input(p,c,&mut f);let b=crate::color::pipeline::input(p,c,&mut g2);source_linear=true;if a.is_err()||b.is_err(){f.d.fill(f32::NAN);}}
                    let q = (prop(c, "flow_quality", lt) as usize).clamp(3,6);
                    let src = Frame { w, h, d: f.d.clone() }; f = crate::flow::flow_blur(&src, &g2, fb.clamp(0.,2.), q); } }
        },
    }
    if p.color_pipeline.enabled && !source_linear && !matches!(c.source, Source::Precomp{..}|Source::Adjustment) {
        if crate::color::pipeline::input(p,c,&mut f).is_err(){f.d.fill(f32::NAN);}
    }
    f
}

fn prop(c: &Clip, k: &str, lt: f64) -> f64 { c.props.get(k).map(|x| x.at(lt)).unwrap_or(0.0) }

/// Inverse-mapped transform (scale/rotate/translate around centre) with alpha-correct bilinear sampling and
/// supersampling (2x2 for any resample, up to 4x4 when minifying) so downscaled and rotated layers are antialiased.
/// Alpha bounding box (x0,y0,x1,y1) of a frame, None if fully transparent.
fn alpha_bbox(src: &Frame) -> Option<(usize, usize, usize, usize)> { let (w, h) = (src.w, src.h); let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize); let mut any = false;
    for y in 0..h { let row = &src.d[y * w * 4..(y + 1) * w * 4]; let mut first = None; let mut last = 0; for x in 0..w { if row[x * 4 + 3] > 0.0 { if first.is_none() { first = Some(x); } last = x; } } if let Some(f0) = first { any = true; x0 = x0.min(f0); x1 = x1.max(last); y0 = y0.min(y); y1 = y1.max(y); } }
    if any { Some((x0, y0, x1, y1)) } else { None } }
fn transform(src: &Frame, scale: f64, rot_deg: f64, dx: f64, dy: f64) -> Frame { transform_bb(src, alpha_bbox(src), scale, rot_deg, dx, dy) }
thread_local! { static FAST_NS: std::cell::Cell<bool> = Default::default(); }
thread_local! { static LAST_RECT: std::cell::Cell<Option<(usize, usize, usize, usize)>> = Default::default(); }
fn transform_bb(src: &Frame, bb: Option<(usize, usize, usize, usize)>, scale: f64, rot_deg: f64, dx: f64, dy: f64) -> Frame {
    if (scale - 1.0).abs() < 1e-9 && rot_deg == 0.0 && dx == 0.0 && dy == 0.0 { return Frame { w: src.w, h: src.h, d: src.d.clone() }; }
    let (w, h) = (src.w, src.h); let mut o = Frame::new(w, h);
    let (cx, cy) = (w as f64/2.0, h as f64/2.0); let r = rot_deg.to_radians(); let (s, co) = (r.sin(), r.cos());
    let inv = 1.0 / scale.max(1e-6);
    let pure_shift = (scale - 1.0).abs() < 1e-9 && rot_deg == 0.0 && dx.fract() == 0.0 && dy.fract() == 0.0;
    let ns = if pure_shift || FAST_NS.with(|l| l.get()) { 1 } else { ((inv.ceil() as usize).max(2)).min(4) }; let nsf = ns as f64; let wgt = 1.0 / (ns * ns) as f32;
    // only visit output pixels that can receive content: forward-map the source's opaque bounding box
    let (bx0, by0, bx1, by1) = match bb { Some(b) => b, None => return o };
    let (mut ox0, mut oy0, mut ox1, mut oy1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (qx, qy) in [(bx0 as f64 - 1.0, by0 as f64 - 1.0), (bx1 as f64 + 2.0, by0 as f64 - 1.0), (bx0 as f64 - 1.0, by1 as f64 + 2.0), (bx1 as f64 + 2.0, by1 as f64 + 2.0)] {
        let (ux, uy) = ((qx - cx) * scale, (qy - cy) * scale); let fx = ux * co - uy * s + cx + dx; let fy = ux * s + uy * co + cy + dy; ox0 = ox0.min(fx); oy0 = oy0.min(fy); ox1 = ox1.max(fx); oy1 = oy1.max(fy); }
    let (rx0, ry0) = ((ox0.floor() - 2.0).max(0.0) as usize, (oy0.floor() - 2.0).max(0.0) as usize); let (rx1, ry1) = (((ox1.ceil() + 2.0).min(w as f64 - 1.0)).max(0.0) as usize, ((oy1.ceil() + 2.0).min(h as f64 - 1.0)).max(0.0) as usize);
    if ox1 < 0.0 || oy1 < 0.0 || ox0 > w as f64 || oy0 > h as f64 { return o; }
    LAST_RECT.with(|l| l.set(Some((rx0, ry0, rx1, ry1))));
    for y in ry0..=ry1 { for x in rx0..=rx1 {
        let mut acc = [0f32; 4];
        for sy_ in 0..ns { for sx_ in 0..ns {
            let px = x as f64 + (sx_ as f64 + 0.5) / nsf - 0.5 - cx - dx; let py = y as f64 + (sy_ as f64 + 0.5) / nsf - 0.5 - cy - dy;
            let sx = (px*co + py*s)*inv + cx; let sy = (-px*s + py*co)*inv + cy;
            if sx < -0.5 || sy < -0.5 || sx > w as f64 - 0.5 || sy > h as f64 - 0.5 { continue; }
            let (sx, sy) = (sx.clamp(0.0, (w-1) as f64), sy.clamp(0.0, (h-1) as f64));
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize); let (x1, y1) = ((x0+1).min(w-1), (y0+1).min(h-1));
            let (fx, fy) = ((sx - x0 as f64) as f32, (sy - y0 as f64) as f32);
            let wts = [(x0, y0, (1.0-fx)*(1.0-fy)), (x1, y0, fx*(1.0-fy)), (x0, y1, (1.0-fx)*fy), (x1, y1, fx*fy)];
            for (qx, qy, k) in wts { let i = (qy*w+qx)*4; let a = src.d[i+3]; let ka = k * a; acc[0] += src.d[i]*ka; acc[1] += src.d[i+1]*ka; acc[2] += src.d[i+2]*ka; acc[3] += ka; }
        }}
        let a = acc[3]; let oi = (y*w+x)*4; if a > 1e-6 { o.d[oi] = acc[0]/a; o.d[oi+1] = acc[1]/a; o.d[oi+2] = acc[2]/a; o.d[oi+3] = a * wgt; }
    }}
    o
}

fn blend(dst: &mut Frame, src: &Frame, opacity: f32, mode: &str, mask: Option<&dyn Fn(usize, usize) -> f32>) {
    let w = dst.w; let m_id = match mode { "add" => 1, "multiply" => 2, "screen" => 3, "overlay" => 4, _ => 0 };
    for y in 0..dst.h {
        let srow = &src.d[y * w * 4..(y + 1) * w * 4];
        // skip the transparent margins of the row (most layers are small)
        let (mut x0, mut x1) = (0usize, w); while x0 < w && srow[x0 * 4 + 3] <= 0.0 { x0 += 1; } if x0 == w { continue; } while x1 > x0 && srow[(x1 - 1) * 4 + 3] <= 0.0 { x1 -= 1; }
        for x in x0..x1 { let i = (y * w + x) * 4;
            let a = srow[x * 4 + 3] * opacity * if let Some(m) = mask { m(x, y) } else { 1.0 }; if a <= 0.0 { continue; }
            for j in 0..3 {
                let (b, s) = (dst.d[i + j], srow[x * 4 + j]);
                let r = match m_id { 1 => b + s, 2 => b * s, 3 => 1.0 - (1.0 - b) * (1.0 - s), 4 => if b < 0.5 { 2.0 * b * s } else { 1.0 - 2.0 * (1.0 - b) * (1.0 - s) }, _ => s };
                dst.d[i + j] = b * (1.0 - a) + r * a;
            }
            dst.d[i + 3] = dst.d[i + 3] + a * (1.0 - dst.d[i + 3]);
        }
    }
}

pub fn render_frame(p: &Project, t: f64, cache: &Cache) -> Result<Frame, String> { render_frame_opts(p, t, cache, false) }

/// `transparent`: start from a clear canvas (used for precomps) instead of opaque black.
pub fn render_frame_opts(p: &Project, t: f64, cache: &Cache, transparent: bool) -> Result<Frame, String> {
    if !t.is_finite(){return Err("render time must be finite".into())}
    p.color_pipeline.validate()?;
    for c in flat_clips(&p.tracks,0.) {for e in c.effects {if let Some(w)=e.strings.get("bound_linear_gamut"){let g=w.parse::<usize>().map_err(|_|"bad bound linear gamut")?;if !p.color_pipeline.enabled||g!=p.color_pipeline.working_gamut||e.params.get("gamut").map(|v|(v.base,v.keys.is_empty(),v.expr.is_empty()))!=Some((g as f64,true,true)){return Err("managed linear effect no longer matches working gamut; re-add it".into())}}if let Some(white)=e.strings.get("wrapper_reference_white"){if white.parse::<f64>().ok()!=Some(p.color_pipeline.reference_white){return Err("effect wrapper reference white changed; reconfigure wrapper".into())}}if let Some(w)=e.strings.get("wrapper_working_gamut"){if !p.color_pipeline.enabled||w.parse::<usize>().ok()!=Some(p.color_pipeline.working_gamut){return Err("effect encoding wrapper no longer matches project working gamut; reconfigure or clear it".into())}}}}
    let (w, h) = (p.width as usize, p.height as usize);
    let mut out = Frame::new(w, h);
    if !transparent { for i in 0..w*h { out.d[i*4+3] = 1.0; } } // opaque black canvas
    let seed = (t * p.fps).round() as u64;
    let hidden: std::collections::HashSet<&str> = p.tracks.iter().flat_map(|tr| tr.clips.iter()).filter_map(|c| c.mask.as_ref().and_then(|m| m.matte.as_ref()).filter(|m| m.hide_source).map(|m| m.source.as_str())).collect();
    for tr in &p.tracks { for c in &tr.clips {
        if t < c.start || t >= c.start + c.duration || (hidden.contains(c.id.as_str()) && MATTE_DEPTH.with(|d| d.get()) == 0) { continue; }
        crate::guard::check();
        let lt0 = if c.posterize_fps > 0.0 { (((t - c.start) * c.posterize_fps + 1e-6).floor() / c.posterize_fps).max(0.0) } else { t - c.start };
        let (ne, esp) = match &c.echo { Some(e) if !matches!(c.source, Source::Adjustment) => (e.count.min(64) as usize, e.spacing), _ => (0, 0.0) };
        let avg = ne > 0 && matches!(c.echo.as_ref().map(|e| e.mode.as_str()), Some("average")); let mut eacc: Option<Frame> = if avg { Some(Frame::new(w, h)) } else { None };
        for ek in (0..=ne).rev() {
        let lt = lt0 - ek as f64 * esp; if lt < 0.0 || lt >= c.duration { continue; }
        if let Source::Adjustment = c.source {
            let mut adj = Frame { w, h, d: out.d.clone() };
            for e in &c.effects { effects::apply(&e.kind, &e.params, &e.strings, &mut adj, lt, seed)?; }
            let op = prop(c, "opacity", lt) as f32; let mk: Option<Frame> = c.mask.as_ref().map(|m| { let mut mf = Frame::new(w, h); for i in 0..w*h { mf.d[i*4] = out.d[i*4]; mf.d[i*4+1] = out.d[i*4+1]; mf.d[i*4+2] = out.d[i*4+2]; mf.d[i*4+3] = 1.0; } apply_mask(&mut mf, m, lt); { let refs = |id: &str| render_clip_alone(p, id, t, cache); let ctx = crate::masks::Ctx { refs: &refs, fps: p.fps }; crate::masks::apply_stack(&mut mf, &m.layers, lt, &ctx); } mf });
            for i in 0..w*h { let k = op * mk.as_ref().map(|m| m.d[i*4+3]).unwrap_or(1.0); for j in 0..3 { out.d[i*4+j] = out.d[i*4+j] * (1.0 - k) + adj.d[i*4+j] * k; } }
            continue;
        }
        let tp=std::time::Instant::now(); let mut f = source_frame(p, c, lt, cache); prof("source", tp);
        let tp=std::time::Instant::now(); apply_effects_masked(p, c, &mut f, lt, seed, t, cache)?; prof("effects", tp);
        if let Some(m) = &c.mask { apply_mask(&mut f, m, lt); if !m.layers.is_empty() { let refs = |id: &str| render_clip_alone(p, id, t, cache); let ctx = crate::masks::Ctx { refs: &refs, fps: p.fps }; crate::masks::apply_stack(&mut f, &m.layers, lt, &ctx); } }
        let mut op = prop(c, "opacity", lt) as f32;
        let mut c_blend = c.blend.as_str();
        if let (Some(e), true) = (&c.echo, ek > 0) { match e.mode.as_str() { "average" => {} m => { op *= e.decay.clamp(0.0, 1.0).powi(ek as i32) as f32; if m == "add" { c_blend = "add"; } else if m == "screen" { c_blend = "screen"; } } } }
                let (mut dx, mut dy) = (prop(c, "x", lt), prop(c, "y", lt));
        let mut mask: Option<Box<dyn Fn(usize, usize) -> f32>> = None;
        let (mut sc, mut rot_extra) = (prop(c, "scale", lt), 0.0);
        let mp_at = |l2: f64| -> (f64, f64, f64) { match &c.motion_path { Some(mp) => { let (x, y, a) = crate::motion::sample(mp, prop(c, "path", l2), w, h); (x, y, if mp.auto_orient { a + mp.orient_offset } else { 0.0 }) } None => (0.0, 0.0, 0.0) } };
        let (mp0x, mp0y, mp0a) = mp_at(lt); dx += mp0x; dy += mp0y; rot_extra += mp0a; let mut tplan: Option<crate::transitions::Plan> = None;
        if let Some(tr_in) = &c.transition_in { if lt < tr_in.duration {
            let pl = crate::transitions::plan(tr_in, lt, w, h); op *= pl.op; dx += pl.dx; dy += pl.dy; sc *= pl.scale; rot_extra += pl.rot;
            if let Some((col, a)) = pl.dip { for i in 0..w * h { for j in 0..3 { out.d[i*4+j] = out.d[i*4+j] * (1.0 - a) + col[j] * a; } out.d[i*4+3] = out.d[i*4+3] + a * (1.0 - out.d[i*4+3]); } }
            mask = None; tplan = Some(pl);
        }}
        let mut f = f; if let Some(pl) = tplan.as_mut() { mask = pl.mask.take(); if let Some(po) = &pl.post { po(&mut f); } }
        let f = if let Some(mut pf) = crate::planar::pin_render(c, &f, lt, p.fps) { let adaptive = p.occluders.get(&c.id).map(|s| s.adaptive).unwrap_or(false); if adaptive { if let Some(sp) = p.occluders.get(&c.id) { if let Some(m) = crate::planar::adaptive_matte(c, sp, p.fps, t, w, h) { for i in 0..w * h { pf.d[i * 4 + 3] *= m[i]; } } } } if !adaptive && crate::planar::anchor_src(&c.id).is_none() { if let Some(sp) = p.occluders.get(&c.id) { crate::planar::rebuild_anchor(&c.id, sp, w, h); } } if let Some((sid, sstart)) = crate::planar::anchor_src(&c.id).filter(|_| !adaptive) { if let (Some(fr), Some(q)) = (cache.frames_of(&sid), crate::planar::pin_quad(c, lt)) { if let Some(cur) = fr.get(((t - sstart).max(0.0) * p.fps) as usize) { let g = prop(c, "pin_gain", lt) as f32; if let Some(m) = crate::planar::occluder_matte(&c.id, cur, &q, if g > 0.0 { g } else { 1.0 }) { for i in 0..w * h { pf.d[i * 4 + 3] *= m[i]; } } } } } pf } else if let Some(cs) = &c.corners { corner_pin(&f, cs) } else { f };
        let (rxv, ryv, pzv) = (prop(c, "rot_x", lt), prop(c, "rot_y", lt), prop(c, "pos_z", lt));
        if rxv != 0.0 || ryv != 0.0 || pzv != 0.0 {
            let cam = crate::three_d::Camera::from_props(&p.camera, t); let pose = crate::three_d::Pose { pos: [dx, dy, pzv], rot: [rxv, ryv, prop(c, "rotation", lt) + rot_extra], scale: sc };
            let f3 = crate::three_d::render_layer(&f, &cam, &pose, prop(c, "backface", lt) != 0.0);
            blend(&mut out, &f3, op, c_blend, mask.as_deref()); if let Some(pl) = &tplan { if let Some(ov) = &pl.overlay { ov(&mut out); } } continue;
        }
        let moving = c.motion_blur > 0.0 && ek == 0 && { let sh0 = c.motion_blur / 360.0 / p.fps; ["x", "y", "scale", "rotation"].iter().any(|k| (prop(c, k, lt - sh0 * 0.5) - prop(c, k, lt)).abs() > 1e-9 || (prop(c, k, lt + sh0 * 0.5) - prop(c, k, lt)).abs() > 1e-9) };
        let tp=std::time::Instant::now(); let bbf = alpha_bbox(&f); let f = if moving { let n = ((c.motion_blur / 22.5).ceil() as usize).clamp(4, 16); let sh = c.motion_blur / 360.0 / p.fps; let mut acc = Frame::new(w, h); let mut wsum = vec![0f32; w * h]; let mut u: Option<(usize, usize, usize, usize)> = None;
            for k in 0..n { let o = (k as f64 + 0.5) / n as f64 - 0.5; let lt2 = lt + o * sh; let (mut dx2, mut dy2) = (prop(c, "x", lt2), prop(c, "y", lt2)); dx2 += dx - prop(c, "x", lt) - mp0x; dy2 += dy - prop(c, "y", lt) - mp0y; let (mp2x, mp2y, mp2a) = mp_at(lt2); dx2 += mp2x; dy2 += mp2y;
                let mut sc2 = prop(c, "scale", lt2); if sc != prop(c, "scale", lt) { sc2 *= sc / prop(c, "scale", lt).max(1e-9); } LAST_RECT.with(|l| l.set(None)); FAST_NS.with(|l| l.set(true)); let tf = transform_bb(&f, bbf, sc2, prop(c, "rotation", lt2) + rot_extra - mp0a + mp2a, dx2, dy2); FAST_NS.with(|l| l.set(false));
                if let Some((rx0, ry0, rx1, ry1)) = LAST_RECT.with(|l| l.get()) { u = Some(match u { Some((a, b, c2, d)) => (a.min(rx0), b.min(ry0), c2.max(rx1), d.max(ry1)), None => (rx0, ry0, rx1, ry1) });
                for y in ry0..=ry1 { for x in rx0..=rx1 { let i = y * w + x; let a = tf.d[i * 4 + 3]; if a > 0.0 { for j in 0..3 { acc.d[i * 4 + j] += tf.d[i * 4 + j] * a; } acc.d[i * 4 + 3] += a; wsum[i] += a; } } } } }
            if let Some((rx0, ry0, rx1, ry1)) = u { for y in ry0..=ry1 { for x in rx0..=rx1 { let i = y * w + x; if wsum[i] > 0.0 { for j in 0..3 { acc.d[i * 4 + j] /= wsum[i]; } } acc.d[i * 4 + 3] /= n as f32; } } } acc }
            else { transform_bb(&f, bbf, sc, prop(c, "rotation", lt) + rot_extra, dx, dy) };
        prof("transform", tp);
        let mut f = f; if let Some(mt) = c.mask.as_ref().and_then(|m| m.matte.as_ref()) { if let Some(src) = render_clip_alone(p, &mt.source, t, cache) { let cov = crate::masks::matte_coverage(mt, &src); for i in 0..w * h { f.d[i * 4 + 3] *= cov[i]; } } }
        if let Some(cf) = tplan.as_ref().and_then(|pl| pl.compose.as_ref()) { out = cf(&out, &f); continue; }
        if let Source::ShardBurst(_) = &c.source { if let Some(d) = crate::shards::take_disp() { crate::shards::refract_canvas(&mut out, &d, &f); } }
        if let Some(acc) = eacc.as_mut() { let wgt = op / (ne + 1) as f32; for i in 0..w * h { let a = f.d[i * 4 + 3] * wgt; if a > 0.0 { for j in 0..3 { acc.d[i * 4 + j] += f.d[i * 4 + j] * a; } acc.d[i * 4 + 3] += a; } } continue; }
        let tp=std::time::Instant::now(); blend(&mut out, &f, op, c_blend, mask.as_deref()); prof("blend", tp);
        if let Some(pl) = &tplan { if let Some(ov) = &pl.overlay { ov(&mut out); } }
        }
        if let Some(mut acc) = eacc { for i in 0..w * h { let a = acc.d[i * 4 + 3]; if a > 1e-6 { for j in 0..3 { acc.d[i * 4 + j] /= a; } } } blend(&mut out, &acc, 1.0, &c.blend, None); }
    }}
    prof_dump();
    if out.d.iter().any(|v|!v.is_finite()){return Err("render produced nonfinite pixels or invalid input color settings".into())}
    Ok(out)
}

pub fn to_rgba8(f: &Frame) -> Vec<u8> { f.d.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect() }

fn image_encoder_command(w:usize,h:usize,path:&str)->Command{let mut cmd=Command::new("ffmpeg");cmd.args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s",&format!("{w}x{h}"),"-i","-","-frames:v","1",path]);cmd}
fn write_image_command(f:&Frame,cmd:&mut Command,deadline:std::time::Duration)->Result<(),String>{let size=f.w.checked_mul(f.h).and_then(|n|n.checked_mul(4)).filter(|n|*n>0&&*n==f.d.len()).ok_or("invalid image frame shape")?;crate::guard::reserve(size);let mut encoder=crate::color::process::EncodeStream::new(cmd,size,deadline)?;encoder.write(to_rgba8(f))?;encoder.finish()}
pub(crate) fn write_image(f:&Frame,path:&str)->Result<(),String>{write_image_command(f,&mut image_encoder_command(f.w,f.h,path),crate::color::process::encode_deadline(1))}

pub fn render_png(p: &Project, t: f64, path: &str, cache: &mut Cache) -> Result<(), String> {
    if p.color_pipeline.enabled && [6,7].contains(&p.color_pipeline.output_transfer) {return Err("HDR output cannot be shown as an SDR PNG; select an SDR preview output or render_high_bit_still".into())}
    prepare_at(p, cache, Some(t))?;
    let mut f = render_frame(p, t, cache)?;
    crate::color::pipeline::output(p,&mut f)?;
    write_image(&f,path)
}

fn legacy_encoder_command(p:&Project,path:&str,crf:u32)->Command{let mut cmd=Command::new("ffmpeg");cmd.args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s",&format!("{}x{}",p.width,p.height),"-r",&format!("{}",p.fps),"-i","-","-c:v","libx264","-pix_fmt","yuv420p","-crf",&crf.to_string(),path]);cmd}

pub fn export(p: &Project, path: &str, crf: u32) -> Result<f64, String> {
    if p.color_pipeline.enabled && [6,7].contains(&p.color_pipeline.output_transfer) { return Err("HDR delivery requires export_high_bit; legacy export is 8-bit SDR".into()); }
    let mut cache = Cache::new();
    let n = (p.duration() * p.fps).round() as usize;
    let mut command=legacy_encoder_command(p,path,crf);
    let frame_bytes=(p.width as usize).checked_mul(p.height as usize).and_then(|v|v.checked_mul(4)).ok_or("legacy encoder frame overflow")?;
    let mut encoder=crate::color::process::EncodeStream::new(&mut command,frame_bytes,crate::color::process::encode_deadline(n))?;
    let nt = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(2).max(1);
    let mut i = 0;
    while i < n {
        crate::guard::check();
        let mut end = (i + nt * 4).min(n);
        loop{let last=(end-1) as f64/p.fps;match prepare_export_range(p,&mut cache,i as f64/p.fps,last){Ok(())=>break,Err(e)if end-i>1&&(e.contains("cache budget")||e.contains("memory cap")||e.contains("bounded cache"))=>{end=i+(end-i).div_ceil(2);cache=Cache::new();},Err(e)=>return Err(e)}}
        // work queue: nt workers pull frame indices, so one slow frame (shards) no longer idles the other cores
        let next = std::sync::atomic::AtomicUsize::new(i); let slots: Vec<std::sync::Mutex<Option<Result<Vec<u8>, String>>>> = (i..end).map(|_| std::sync::Mutex::new(None)).collect();
        std::thread::scope(|sc| { for _ in 0..nt.min(end - i) { let (cache, next, slots) = (&cache, &next, &slots); sc.spawn(move || loop { let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst); if k >= end { break; }
            let res = render_frame(p, k as f64 / p.fps, cache).and_then(|mut f| {crate::color::pipeline::output(p,&mut f)?;Ok(to_rgba8(&f))}); *slots[k - i].lock().unwrap() = Some(res); }); } });
        let frames: Vec<Result<Vec<u8>, String>> = slots.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect();
        for f in frames { encoder.write(f?)?; }
        i = end;
    }
    encoder.finish()?;
    // mux mixed audio
    let pcm = crate::audio::mix(p);
    if pcm.iter().any(|v|v.abs()>1e-6){crate::color::mux::legacy_audio(path,&pcm,n)?}
    Ok(n as f64 / p.fps)
}


thread_local! { static MATTE_DEPTH: std::cell::Cell<u32> = std::cell::Cell::new(0); }
/// Render one clip alone (full transforms/effects) on a transparent canvas: matte sources and difference references.
pub fn render_clip_alone(p: &Project, id: &str, t: f64, cache: &Cache) -> Option<Frame> {
    if MATTE_DEPTH.with(|d| d.get()) >= 3 { return None; }
    let c = p.tracks.iter().flat_map(|tr| tr.clips.iter()).find(|c| c.id == id)?.clone();
    if t < c.start || t >= c.start + c.duration { return Some(Frame::new(p.width as usize, p.height as usize)); }
    let mut c = c; if let Some(m) = c.mask.as_mut() { m.matte = None; }
    let sub = Project { occluders: p.occluders.clone(), color_pipeline: p.color_pipeline.clone(), tracks: vec![crate::model::Track { id: "matte".into(), clips: vec![c], locked: false, muted: false, name: String::new() }], assets: p.assets.clone(), camera: p.camera.clone(), markers: vec![], name: String::new(), width: p.width, height: p.height, fps: p.fps, next_id: 0 };
    MATTE_DEPTH.with(|d| d.set(d.get() + 1)); let r = render_frame_opts(&sub, t, cache, true).ok(); MATTE_DEPTH.with(|d| d.set(d.get() - 1)); r
}

/// Effects with a per-effect mask (strings.mask_layer = id of a layer in the clip's mask stack) only apply where that layer covers.
fn apply_effects_masked(p: &Project, c: &Clip, f: &mut Frame, lt: f64, seed: u64, t: f64, cache: &Cache) -> Result<(), String> {
    if !c.effects.iter().any(|e| e.strings.contains_key("mask_layer")) { return apply_effects_local(&c.effects, f, lt, seed); }
    let mut i = 0;
    while i < c.effects.len() {
        let e = &c.effects[i];
        if let Some(mid) = e.strings.get("mask_layer") {
            let before = Frame { w: f.w, h: f.h, d: f.d.clone() };
            apply_effects_local(std::slice::from_ref(e), f, lt, seed)?;
            let refs = |id: &str| render_clip_alone(p, id, t, cache); let ctx = crate::masks::Ctx { refs: &refs, fps: p.fps };
            if let Some(l) = c.mask.as_ref().and_then(|m| m.layers.iter().find(|l| &l.id == mid)) {
                let cov = crate::masks::layer_coverage(l, &before, lt, &ctx);
                for k in 0..f.w * f.h { let a = cov[k]; if a < 1.0 { for j in 0..4 { f.d[k * 4 + j] = before.d[k * 4 + j] * (1.0 - a) + f.d[k * 4 + j] * a; } } }
            }
            i += 1;
        } else { let j = (i..c.effects.len()).find(|&j| c.effects[j].strings.contains_key("mask_layer")).unwrap_or(c.effects.len()); apply_effects_local(&c.effects[i..j], f, lt, seed)?; i = j; }
    }
    Ok(())
}

fn apply_mask(f: &mut Frame, m: &Mask, t: f64) {
    if m.shape == "stack" { return; }
    let g = |k: &str, d: f64| m.props.get(k).map(|p| p.at(t)).unwrap_or(d) as f32;
    let (cx, cy, mw, mh, feather, rot) = (g("cx",0.5), g("cy",0.5), g("w",0.5), g("h",0.5), g("feather",0.02).max(0.0005), g("rotation",0.0).to_radians());
    let (w, h) = (f.w as f32, f.h as f32); let (s, c) = (rot.sin(), rot.cos());
    for y in 0..f.h { for x in 0..f.w {
        let (nx, ny) = (x as f32 / w - cx, y as f32 / h - cy);
        let (ux, uy) = ((nx*c + ny*s) * (w / h), (-nx*s + ny*c));
        let (ux, uy) = (ux / (w / h), uy);
        let d = match m.shape.as_str() {
            "ellipse" => { let e = ((ux/(mw/2.0)).powi(2) + (uy/(mh/2.0)).powi(2)).sqrt(); (1.0 - e) * (mw.min(mh)/2.0) }
            "linear" => -ux,
            _ => (mw/2.0 - ux.abs()).min(mh/2.0 - uy.abs()),
        };
        let mut a = (d / feather + 0.5).clamp(0.0, 1.0); if m.invert { a = 1.0 - a; }
        f.d[(y*f.w+x)*4+3] *= a;
    }}
}

/// Perspective warp of the whole frame by moving its 4 corners (TL,TR,BR,BL) by pixel offsets.
fn corner_pin(src: &Frame, off: &[[f64; 2]; 4]) -> Frame {
    let (w, h) = (src.w as f64, src.h as f64);
    let from = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]; let to: Vec<[f64; 2]> = (0..4).map(|i| [from[i][0] + off[i][0], from[i][1] + off[i][1]]).collect();
    // homography mapping `to` (dest) -> `from` (source): solve 8x8
    let mut a = [[0f64; 9]; 8];
    for i in 0..4 { let (x, y) = (to[i][0], to[i][1]); let (u, v) = (from[i][0], from[i][1]);
        a[i*2] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u]; a[i*2+1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v]; }
    for c in 0..8 { let mut m = c; for r in c + 1..8 { if a[r][c].abs() > a[m][c].abs() { m = r; } } a.swap(c, m); let d = a[c][c]; if d.abs() < 1e-12 { return Frame { w: src.w, h: src.h, d: src.d.clone() }; }
        for k in c..9 { a[c][k] /= d; } for r in 0..8 { if r != c { let f = a[r][c]; for k in c..9 { a[r][k] -= f * a[c][k]; } } } }
    let hm: Vec<f64> = (0..8).map(|i| a[i][8]).collect();
    let mut o = Frame::new(src.w, src.h);
    for y in 0..src.h { for x in 0..src.w { let (xf, yf) = (x as f64 + 0.5, y as f64 + 0.5); let dnm = hm[6] * xf + hm[7] * yf + 1.0; let sx = (hm[0] * xf + hm[1] * yf + hm[2]) / dnm; let sy = (hm[3] * xf + hm[4] * yf + hm[5]) / dnm;
        if sx < 0.0 || sy < 0.0 || sx > w - 1.0 || sy > h - 1.0 { continue; } let (x0, y0) = (sx.floor() as usize, sy.floor() as usize); let (x1, y1) = ((x0 + 1).min(src.w - 1), (y0 + 1).min(src.h - 1)); let (fx, fy) = ((sx - x0 as f64) as f32, (sy - y0 as f64) as f32);
        for j in 0..4 { let p00 = src.d[(y0 * src.w + x0) * 4 + j]; let p10 = src.d[(y0 * src.w + x1) * 4 + j]; let p01 = src.d[(y1 * src.w + x0) * 4 + j]; let p11 = src.d[(y1 * src.w + x1) * 4 + j]; o.d[(y * src.w + x) * 4 + j] = (p00 * (1.0 - fx) + p10 * fx) * (1.0 - fy) + (p01 * (1.0 - fx) + p11 * fx) * fy; } } }
    o
}

/// Dynamic text tokens, resolved per frame at render time: `{tc:S}` timecode HH:MM:SS:FF offset by S seconds,
/// `{bar:BPM:S}` musical position "BAR bb.b.s" at BPM offset by S seconds, `{t:S}` seconds with 2 decimals, `{pct:A:B}` percent 0..100 from timeline time A to B.
pub fn dyn_text(text: &str, gt: f64, fps: f64) -> String {
    if !text.contains('{') { return text.to_string(); }
    let mut out = String::new(); let mut rest = text;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]); let tail = &rest[i + 1..];
        if let Some(j) = tail.find('}') {
            let parts: Vec<&str> = tail[..j].split(':').collect(); let f = |k: usize| parts.get(k).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            match parts[0] {
                "tc" => { let g = gt + f(1); out.push_str(&format!("{:02}:{:02}:{:02}:{:02}", (g / 3600.0) as u32, ((g / 60.0) as u32) % 60, (g as u32) % 60, (g.fract() * fps) as u32)); }
                "bar" => { let g = gt + f(2); let bars = g / (60.0 / f(1).max(1.0)); out.push_str(&format!("{:02}.{}.{}", (bars / 4.0) as u32 + 1, (bars as u32) % 4 + 1, (bars.fract() * 4.0) as u32 + 1)); }
                "t" => out.push_str(&format!("{:.2}", gt + f(1))),
                "pct" => { let (a, b) = (f(1), f(2)); out.push_str(&format!("{}", (((gt - a) / (b - a).max(1e-6)).clamp(0.0, 1.0) * 100.0).round() as u32)); }
                _ => { out.push('{'); out.push_str(&tail[..=j]); rest = &tail[j + 1..]; continue; }
            }
            rest = &tail[j + 1..];
        } else { out.push('{'); rest = tail; }
    }
    out.push_str(rest); out
}

/// Local (spatially invariant) effects run on the layer's alpha bounding box plus a margin instead of the full canvas.
/// Big win for small shapes with glow/blur/shadow (echo ghosts, particles, UI plates).
fn apply_effects_local(effs: &[crate::model::Effect], f: &mut Frame, lt: f64, seed: u64) -> Result<(), String> {
    const LOCAL: [&str; 7] = ["blur", "outer_glow", "drop_shadow", "glow", "choker", "feather_alpha", "outline"];
    if effs.is_empty() { return Ok(()); }
    let (w, h) = (f.w, f.h);
    let mut bb: Option<(usize, usize, usize, usize)> = None; // x0,y0,x1,y1
    let mut need = effs.iter().any(|e| LOCAL.contains(&e.kind.as_str()));
    if need { let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize); for y in 0..h { let row = &f.d[y * w * 4..(y + 1) * w * 4]; let mut first = None; let mut last = 0; for x in 0..w { if row[x * 4 + 3] > 0.0 { if first.is_none() { first = Some(x); } last = x; } } if let Some(fx) = first { x0 = x0.min(fx); x1 = x1.max(last); y0 = y0.min(y); y1 = y1.max(y); } }
        if x1 < x0 { need = false; } else { bb = Some((x0, y0, x1, y1)); } }
    let mut i = 0;
    while i < effs.len() {
        let e = &effs[i];
        if need && LOCAL.contains(&e.kind.as_str()) {
            // run the whole consecutive run of local effects on one crop
            let mut j = i; let mut mg = 0.0f64; while j < effs.len() && LOCAL.contains(&effs[j].kind.as_str()) { let rad = effs[j].params.get("radius").map(|p| p.at(lt)).unwrap_or(16.0).abs(); mg += rad * 3.0 + 12.0; j += 1; }
            let (x0, y0, x1, y1) = bb.unwrap(); let m = mg.ceil() as usize;
            let (cx0, cy0, cx1, cy1) = (x0.saturating_sub(m), y0.saturating_sub(m), (x1 + m).min(w - 1), (y1 + m).min(h - 1)); let (cw, ch) = (cx1 - cx0 + 1, cy1 - cy0 + 1);
            let mut cf = Frame::new(cw, ch); for y in 0..ch { cf.d[y * cw * 4..(y + 1) * cw * 4].copy_from_slice(&f.d[((y + cy0) * w + cx0) * 4..((y + cy0) * w + cx0 + cw) * 4]); }
            for e2 in &effs[i..j] { effects::apply(&e2.kind, &e2.params, &e2.strings, &mut cf, lt, seed)?; }
            for y in 0..ch { f.d[((y + cy0) * w + cx0) * 4..((y + cy0) * w + cx0 + cw) * 4].copy_from_slice(&cf.d[y * cw * 4..(y + 1) * cw * 4]); }
            i = j; continue;
        }
        effects::apply(&e.kind, &e.params, &e.strings, f, lt, seed)?; i += 1;
    }
    Ok(())
}

thread_local! { static PROF: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>> = Default::default(); }
fn prof(k: &'static str, t: std::time::Instant) { if std::env::var("KINO_PROF").is_ok() { PROF.with(|p| *p.borrow_mut().entry(k).or_insert(0.0) += t.elapsed().as_secs_f64()); } }
pub fn prof_dump() { PROF.with(|p| { let mut p = p.borrow_mut(); if !p.is_empty() { eprintln!("PROF {:?}", *p); p.clear(); } }); }

/// Inspect a mask: renders the clip's source (after effects, before its own mask) and shows the coverage of one layer or the whole stack.
pub fn render_mask_png(p: &Project, clip_id: &str, mask_id: Option<&str>, t: f64, view: &str, path: &str, cache: &mut Cache) -> Result<(), String> {
    prepare_at(p, cache, Some(t))?;
    let c = p.tracks.iter().flat_map(|tr| tr.clips.iter()).find(|c| c.id == clip_id).ok_or("no such clip")?; let lt = t - c.start; let seed = (t * p.fps).round() as u64;
    let (w, h) = (p.width as usize, p.height as usize);
    let mut f = source_frame(p, c, lt, cache); apply_effects_masked(p, c, &mut f, lt, seed, t, cache)?;
    if let Some(m) = &c.mask { let mut tmp = Frame { w, h, d: f.d.clone() }; apply_mask(&mut tmp, m, lt); if m.shape != "stack" { for i in 0..w * h { f.d[i * 4 + 3] = tmp.d[i * 4 + 3]; } } }
    let cache_ref: &Cache = cache; let refs = |id: &str| render_clip_alone(p, id, t, cache_ref); let ctx = crate::masks::Ctx { refs: &refs, fps: p.fps };
    let layers = c.mask.as_ref().map(|m| m.layers.clone()).unwrap_or_default();
    let cov: Vec<f32> = match mask_id { Some(id) => { let l = layers.iter().find(|l| l.id == id).ok_or("no such mask")?; crate::masks::layer_coverage(l, &f, lt, &ctx) } None => crate::masks::stack_coverage(&layers, &f, lt, &ctx).unwrap_or_else(|| vec![1.0; w * h]) };
    let mut o = Frame::new(w, h);
    for i in 0..w * h { let k = cov[i]; match view {
        "overlay" => { let g = 0.35 + 0.0; for j in 0..3 { o.d[i * 4 + j] = f.d[i * 4 + j] * (1.0 - k * g) + if j == 0 { 1.0 } else { 0.0 } * k * g; } }
        "cutout" => { for j in 0..3 { o.d[i * 4 + j] = f.d[i * 4 + j]; } } // straight colour; alpha carries the matte so the PNG is a real transparent cutout
        _ => { for j in 0..3 { o.d[i * 4 + j] = k; } } } o.d[i * 4 + 3] = if view == "cutout" { k * f.d[i * 4 + 3] } else { 1.0 }; }
    write_image(&o,path)
}

/// Write a colour-wheel visualisation of the optical flow between a media clip's frame at `t` and the next one.
pub fn render_flow_png(p: &Project, clip_id: &str, t: f64, path: &str, cache: &mut Cache) -> Result<f64, String> {
    let c = p.tracks.iter().flat_map(|tr| tr.clips.iter()).find(|c| c.id == clip_id).ok_or("no such clip")?; let (w, h) = (p.width as usize, p.height as usize);
    if !t.is_finite()||t<c.start||t>=c.start+c.duration{return Err("flow time must be within clip".into())}
    let lt0=t-c.start;let last=media_total(c,p.fps).saturating_sub(1);let fidx=|lt:f64|->usize{let d=c.display_time(lt.max(0.));media_index(c,d,p.fps).min(last)};
    let(i0,mut i1)=(fidx(lt0),fidx(lt0+1./p.fps));let mut lt1=lt0+1./p.fps;let mut swapped=false;
    if i1==i0{lt1=lt0-1./p.fps;i1=fidx(lt1);swapped=i1!=i0;}
    if i1==i0{return Err("no motion between displayed frames here (freeze or window end)".into())}
    prepare_range(p,cache,t.min(c.start+lt1),t.max(c.start+lt1))?;
    let fr=cache.frames_of(&c.id).ok_or("not a media clip")?;
    let(a,b)=flow_pair(fr,if swapped{i1}else{i0},if swapped{i0}else{i1},w,h)?;
    let t0=std::time::Instant::now();let(u,v,fw,fh)=crate::flow::flow(&a,&b,5);let secs=t0.elapsed().as_secs_f64();
    let mm = u.iter().zip(&v).map(|(a, b)| (a * a + b * b).sqrt()).fold(0.0f32, f32::max).max(1.0); let viz = crate::flow::flow_viz(&u, &v, fw, fh, mm * 0.8);
    write_image(&viz,path)?;Ok(secs)
}

fn flow_pair(fr:&Frames,a:usize,b:usize,w:usize,h:usize)->Result<(Frame,Frame),String>{
 if !fr.covers(a.min(b),a.max(b)){return Err("active flow frame pair exceeds cache coverage".into())}
 let n=w.checked_mul(h).and_then(|n|n.checked_mul(4)).ok_or("flow dimensions overflow")?;if fr.fs!=n{return Err("flow cache shape mismatch".into())}
 crate::guard::reserve(n.checked_mul(8).ok_or("flow pair allocation overflow")?);
 let mk=|i:usize|->Result<Frame,String>{let d=if let Some(v)=fr.get_float(i){v.to_vec()}else{fr.get(i).ok_or("no flow frame")?.iter().map(|v|*v as f32/255.).collect()};Ok(Frame{w,h,d})};Ok((mk(a)?,mk(b)?))
}
#[cfg(test)]mod flow_pair_tests{use super::*;
#[test]fn flow_pair_keeps_float_and_rejects_clamped_video(){let d=vec![-0.25,4.,0.18,0.5,1.,2.,3.,1.];let f=Frames{still:false,working_linear:true,quantized:Default::default(),linear_buf:Some(d.clone()),buf:vec![],fs:4,first:10};let(a,b)=flow_pair(&f,10,11,1,1).unwrap();assert_eq!(a.d,&d[..4]);assert_eq!(b.d,&d[4..]);assert!(f.quantized.get().is_none());assert!(flow_pair(&f,10,12,1,1).is_err());}
}

#[cfg(test)]mod decode_lifecycle_test{use super::*;
#[test]fn decoder_reaped_on_unwind(){let mut id=0;let r=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let child=Command::new("sleep").arg("30").spawn().unwrap();id=child.id();let _decoder=DecodeProcess(child);panic!("test cancelled decode");}));assert!(r.is_err());assert!(!std::path::Path::new(&format!("/proc/{id}")).exists());}
}
#[cfg(test)]mod legacy_encoder_tests{use super::*;
#[test]fn logged_argv_exactly_preserves_legacy_options(){let mut p=Project::default();p.width=320;p.height=180;p.fps=25.;let cmd=legacy_encoder_command(&p,"proof.mp4",23);let logged:Vec<_>=cmd.get_args().map(|v|v.to_string_lossy().to_string()).collect();assert_eq!(logged,vec!["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s","320x180","-r","25","-i","-","-c:v","libx264","-pix_fmt","yuv420p","-crf","23","proof.mp4"]);}
#[test]fn parallel_out_of_order_arrival_preserves_writer_order(){let path=std::env::temp_dir().join(format!("kino-ordered-{}.bin",std::process::id()));let mut cmd=Command::new("sh");cmd.args(["-c",&format!("exec cat > {}",path.display())]);let mut encoder=crate::color::process::EncodeStream::new(&mut cmd,4,std::time::Duration::from_secs(2)).unwrap();let slots:Vec<_>=(0..4).map(|_|std::sync::Mutex::new(None)).collect();std::thread::scope(|sc|{for k in 0..4{let slots=&slots;sc.spawn(move||{std::thread::sleep(std::time::Duration::from_millis((3-k)*10));*slots[k as usize].lock().unwrap()=Some(vec![k as u8;4]);});}});for slot in slots{encoder.write(slot.into_inner().unwrap().unwrap()).unwrap();}encoder.finish().unwrap();assert_eq!(std::fs::read(&path).unwrap(),vec![0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3]);std::fs::remove_file(path).unwrap();}
}
#[cfg(test)]mod legacy_unwind_test{use super::*;
#[test]fn render_worker_panic_reaps_encoder(){let pidpath=std::env::temp_dir().join(format!("kino-legacy-unwind-pid-{}",std::process::id()));let script=format!("echo $$ > {}; exec sleep 30",pidpath.display());let result=std::panic::catch_unwind(||{let _encoder=crate::color::process::EncodeStream::new(Command::new("sh").args(["-c",&script]),4,std::time::Duration::from_secs(2)).unwrap();std::thread::sleep(std::time::Duration::from_millis(30));std::thread::scope(|sc|{sc.spawn(||panic!("injected render worker failure"));});});assert!(result.is_err());let id=std::fs::read_to_string(&pidpath).unwrap();assert!(!std::path::Path::new(&format!("/proc/{}",id.trim())).exists());std::fs::remove_file(pidpath).unwrap();}
}
#[cfg(test)]mod image_writer_tests{use super::*;
#[test]fn alpha_png_exact_bytes_and_decoded_hash(){use std::hash::{Hash,Hasher};let base=std::env::temp_dir().join(format!("kino-image-base-{}.png",std::process::id()));let after=std::env::temp_dir().join(format!("kino-image-after-{}.png",std::process::id()));let f=Frame{w:32,h:16,d:(0..512).flat_map(|i|[i as f32/511.,0.25,0.75,(i%32) as f32/31.]).collect()};let mut old=image_encoder_command(32,16,base.to_str().unwrap()).stdin(Stdio::piped()).spawn().unwrap();old.stdin.take().unwrap().write_all(&to_rgba8(&f)).unwrap();assert!(old.wait().unwrap().success());write_image(&f,after.to_str().unwrap()).unwrap();assert_eq!(std::fs::read(&base).unwrap(),std::fs::read(&after).unwrap());let decode=|path:&std::path::Path|Command::new("ffmpeg").args(["-v","error","-i"]).arg(path).args(["-f","rawvideo","-pix_fmt","rgba","-"]).output().unwrap().stdout;let a=decode(&base);let b=decode(&after);let hash=|v:&[u8]|{let mut h=std::collections::hash_map::DefaultHasher::new();v.hash(&mut h);h.finish()};assert_eq!(hash(&a),hash(&b));assert_eq!(a,b);assert!(b.chunks_exact(4).any(|p|p[3]>0&&p[3]<255));std::fs::remove_file(base).unwrap();std::fs::remove_file(after).unwrap();}
#[test]fn stuck_image_child_deadline_reaps(){let pid=std::env::temp_dir().join(format!("kino-image-stuck-pid-{}",std::process::id()));let mut cmd=Command::new("sh");cmd.args(["-c",&format!("echo $$ > {}; exec sleep 30",pid.display())]);let f=Frame{w:512,h:512,d:vec![0.25;512*512*4]};let start=std::time::Instant::now();assert!(write_image_command(&f,&mut cmd,std::time::Duration::from_millis(100)).unwrap_err().contains("deadline"));assert!(start.elapsed()<std::time::Duration::from_secs(2));let id=std::fs::read_to_string(&pid).unwrap();assert!(!std::path::Path::new(&format!("/proc/{}",id.trim())).exists());std::fs::remove_file(pid).unwrap();}
#[test]fn image_argv_is_unchanged(){let cmd=image_encoder_command(32,16,"alpha.png");let argv:Vec<_>=cmd.get_args().map(|v|v.to_string_lossy().to_string()).collect();assert_eq!(argv,vec!["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s","32x16","-i","-","-frames:v","1","alpha.png"]);}
}
#[cfg(test)]mod color_tool_image_argv{use super::*;
#[test]fn each_migrated_color_writer_uses_original_argv(){for writer in ["render_aces2_reference","render_gamut_excursions","render_exposure_false_color"]{let path=format!("{writer}.png");let cmd=image_encoder_command(320,180,&path);let args:Vec<_>=cmd.get_args().map(|v|v.to_string_lossy().to_string()).collect();assert_eq!(args,vec!["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s","320x180","-i","-","-frames:v","1",&path],"writer{writer}");}}
}
