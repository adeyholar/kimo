use crate::effects::{self, Frame};
use crate::model::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// Decoded frames for media clips, keyed by clip id. Frames are RGBA8 at project size, project fps,
/// with speed already applied.
/// A decoded window of consecutive frames, contiguous in memory. `first` is the global frame index of buf[0].
pub struct Frames { pub buf: Vec<u8>, pub fs: usize, pub first: usize }
impl Frames {
    pub fn len(&self) -> usize { if self.fs == 0 { 0 } else { self.buf.len() / self.fs } }
    pub fn covers(&self, a: usize, b: usize) -> bool { self.len() > 0 && a >= self.first && b < self.first + self.len() }
    /// Global-index lookup, clamped into the window.
    pub fn get(&self, g: usize) -> Option<&[u8]> { let n = self.len(); if n == 0 { return None; } let i = g.saturating_sub(self.first).min(n - 1); Some(&self.buf[i * self.fs..(i + 1) * self.fs]) }
}
impl std::ops::Index<usize> for Frames { type Output = [u8]; fn index(&self, i: usize) -> &[u8] { &self.buf[i * self.fs..(i + 1) * self.fs] } }
pub struct Cache { frames: HashMap<String, (String, Frames)>, pub size: (u32, u32, f64) }
impl Cache { pub fn new() -> Self { Cache { frames: HashMap::new(), size: (0, 0, 0.0) } } pub fn frames_of(&self, id: &str) -> Option<&Frames> { self.frames.get(id).map(|x| &x.1) } }

pub fn probe(path: &str) -> Result<(u32, u32, f64, f64), String> {
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

const WINDOW: usize = 24;

/// Decode `n` frames starting at global clip frame `a` (windowed: memory stays bounded for any clip length).
fn decode(p: &Project, c: &Clip, path: &str, a: usize, n: usize) -> Result<Frames, String> {
    let (w, h) = (p.width, p.height);
    let (in_pt, speed, span) = if c.speed_curve.is_some() { (c.in_point, 1.0, c.source_time(c.duration)) } else { (c.in_point + a as f64 / p.fps * c.speed, c.speed, n as f64 / p.fps * c.speed) };
    let vf = format!("setpts=PTS/{},fps={},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black@0,format=rgba", speed.max(0.01), p.fps);
    let is_img = ["png","jpg","jpeg","webp","bmp"].iter().any(|e| path.to_lowercase().ends_with(e));
    let loop_args: Vec<&str> = if is_img { vec!["-loop", "1"] } else { vec![] };
    let fs = (w * h * 4) as usize;
    let mut child = Command::new("ffmpeg").args(["-v","error"]).args(&loop_args).args(["-ss",&format!("{in_pt}"),"-t",&format!("{span}"),"-i",path,"-vf",&vf,"-frames:v",&n.max(1).to_string(),"-f","rawvideo","-pix_fmt","rgba","-"])
        .stdout(Stdio::piped()).spawn().map_err(|e| format!("ffmpeg: {e}"))?;
    let mut buf = Vec::with_capacity(fs * n.max(1)); child.stdout.take().unwrap().read_to_end(&mut buf).map_err(|e| e.to_string())?; let _ = child.wait();
    let k = buf.len() / fs; buf.truncate(k * fs);
    Ok(Frames { buf, fs, first: if c.speed_curve.is_some() { 0 } else { a } })
}

pub fn prepare(p: &Project, cache: &mut Cache) -> Result<(), String> { prepare_at(p, cache, None) }

/// Decode media clips (only those active at `t` when given). Re-decodes when clip timing/speed/project size changed.
pub fn prepare_at(p: &Project, cache: &mut Cache, t: Option<f64>) -> Result<(), String> { match t { Some(t) => prepare_range(p, cache, t, t), None => prepare_range(p, cache, f64::MIN, f64::MAX) } }

/// Decode clips overlapping [t0, t1] and evict decoded clips outside it (bounds memory).
pub fn prepare_range(p: &Project, cache: &mut Cache, t0: f64, t1: f64) -> Result<(), String> {
    if cache.size != (p.width, p.height, p.fps) { cache.frames.clear(); cache.size = (p.width, p.height, p.fps); }
    let all = t0 == f64::MIN;
    let active: std::collections::HashSet<String> = p.tracks.iter().flat_map(|tr| tr.clips.iter()).filter(|c| all || (c.start <= t1 && c.start + c.duration > t0)).map(|c| c.id.clone()).collect();
    if !all { cache.frames.retain(|k, _| active.contains(k)); }
    for tr in &p.tracks { for c in &tr.clips {
        let Source::Media { asset } = &c.source else { continue };
        if !active.contains(&c.id) { continue; }
        let total = ((c.duration * p.fps).ceil() as usize).max(1);
        let (ta, tb) = match c.time_mode { TimeMode::Normal => (t0, t1), TimeMode::Reverse => (c.start + c.duration - (t1 - c.start), c.start + c.duration - (t0 - c.start)), TimeMode::Freeze { at } => (c.start + at, c.start + at) };
        let (t0, t1) = (ta, tb);
        let (ia, ib) = if all || c.speed_curve.is_some() { (0, total - 1) } else { ((((t0 - c.start).max(0.0) * p.fps).floor() as usize).min(total - 1), (((t1 - c.start).max(0.0) * p.fps).ceil() as usize).min(total - 1)) };
        let sig = format!("{asset}|{}|{}|{}|{:?}", c.in_point, c.duration, c.speed, c.speed_curve.as_ref().map(|p| serde_json::to_string(p).unwrap_or_default()));
        if let Some((s, fr)) = cache.frames.get(&c.id) { if *s == sig && fr.covers(ia, ib) { continue; } }
        let a = p.assets.get(asset).ok_or(format!("asset {asset} missing"))?;
        if a.width == 0 { cache.frames.insert(c.id.clone(), (sig, Frames { buf: vec![], fs: 0, first: 0 })); continue; }
        // whole-clip decode only for speed-ramped clips (index mapping is in source time); everything else is windowed
        let (start, count) = if c.speed_curve.is_some() || all { (0, total) } else { (ia, (ib - ia + 1).max(WINDOW).min(total - ia)) };
        let fr = decode(p, c, &a.path, start, count)?; cache.frames.insert(c.id.clone(), (sig, fr));
    }}
    Ok(())
}

fn source_frame(p: &Project, c: &Clip, lt_in: f64, cache: &Cache) -> Frame {
    let lt = c.display_time(lt_in);
    let (w, h) = (p.width as usize, p.height as usize);
    let mut f = Frame::new(w, h);
    match &c.source {
        Source::Solid { color } => for i in 0..w*h { f.d[i*4]=color[0] as f32; f.d[i*4+1]=color[1] as f32; f.d[i*4+2]=color[2] as f32; f.d[i*4+3]=1.0; },
        Source::Gradient { c1, c2 } => for y in 0..h { let k = y as f64 / h as f64; for x in 0..w { let i=(y*w+x)*4; for j in 0..3 { f.d[i+j] = (c1[j]*(1.0-k)+c2[j]*k) as f32; } f.d[i+3]=1.0; } },
        Source::Adjustment => {}
        Source::Shape { shape, width, height, fill, stroke_width, stroke_color, sides, corner_radius } => {
            let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0); let (hw, hh) = (width / 2.0, height / 2.0);
            // signed distance (negative inside)
            let sd = |x: f64, y: f64| -> f64 { let (px, py) = (x - cx, y - cy); match shape.as_str() {
                "ellipse" => { let k = ((px / hw).powi(2) + (py / hh).powi(2)).sqrt(); (k - 1.0) * hw.min(hh) }
                "polygon" => { let n = (*sides).max(3) as f64; let r = hw.min(hh); let a = py.atan2(px) + std::f64::consts::FRAC_PI_2; let seg = std::f64::consts::TAU / n; let aa = (a.rem_euclid(seg)) - seg / 2.0; (px * px + py * py).sqrt() * aa.cos() - r * (seg / 2.0).cos() }
                "line" => (py.abs() - hh.max(1.0)).max(px.abs() - hw),
                _ => { let (qx, qy) = (px.abs() - hw + corner_radius, py.abs() - hh + corner_radius); (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - corner_radius } } };
            for y in 0..h { for x in 0..w { let d = sd(x as f64 + 0.5, y as f64 + 0.5); let inside = (0.5 - d).clamp(0.0, 1.0); if inside <= 0.0 && *stroke_width <= 0.0 { continue; }
                let sk = if *stroke_width > 0.0 { (0.5 - (d.abs() - stroke_width / 2.0)).clamp(0.0, 1.0) } else { 0.0 }; let fl = if *stroke_width > 0.0 { (0.5 - (d + stroke_width / 2.0)).clamp(0.0, 1.0) } else { inside };
                let i = (y * w + x) * 4; let a = (fl + sk * (1.0 - fl)).clamp(0.0, 1.0); if a <= 0.0 { continue; }
                for j in 0..3 { f.d[i + j] = ((fill[j] * fl + stroke_color[j] * sk * (1.0 - fl)) / a) as f32; } f.d[i + 3] = a as f32; } }
        }
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
            let _ = crate::text::render_text(&mut f, text, font, *size, *color, *letter_spacing, *stroke_width, *stroke_color, *shadow, align, prop(c, "reveal", lt));
        }
        Source::Media { .. } => if let Some(fr) = cache.frames_of(&c.id) {
            let idx = if c.speed_curve.is_some() { (c.source_time(lt) * p.fps) as usize } else { (lt * p.fps) as usize };
            if let Some(b) = fr.get(idx) { for (i, v) in b.iter().enumerate() { f.d[i] = *v as f32 / 255.0; } }
        },
    }
    f
}

fn prop(c: &Clip, k: &str, lt: f64) -> f64 { c.props.get(k).map(|x| x.at(lt)).unwrap_or(0.0) }

/// Inverse-mapped bilinear transform (scale/rotate/translate around centre).
fn transform(src: &Frame, scale: f64, rot_deg: f64, dx: f64, dy: f64) -> Frame {
    if (scale - 1.0).abs() < 1e-9 && rot_deg == 0.0 && dx == 0.0 && dy == 0.0 { return Frame { w: src.w, h: src.h, d: src.d.clone() }; }
    let (w, h) = (src.w, src.h); let mut o = Frame::new(w, h);
    let (cx, cy) = (w as f64/2.0, h as f64/2.0); let r = rot_deg.to_radians(); let (s, co) = (r.sin(), r.cos());
    let inv = 1.0 / scale.max(1e-6);
    for y in 0..h { for x in 0..w {
        let px = x as f64 - cx - dx; let py = y as f64 - cy - dy;
        let sx = (px*co + py*s)*inv + cx; let sy = (-px*s + py*co)*inv + cy;
        if sx < 0.0 || sy < 0.0 || sx > (w-1) as f64 || sy > (h-1) as f64 { continue; }
        let (x0, y0) = (sx.floor() as usize, sy.floor() as usize); let (x1, y1) = ((x0+1).min(w-1), (y0+1).min(h-1));
        let (fx, fy) = ((sx - x0 as f64) as f32, (sy - y0 as f64) as f32);
        for j in 0..4 {
            let a = src.d[(y0*w+x0)*4+j]*(1.0-fx) + src.d[(y0*w+x1)*4+j]*fx;
            let b = src.d[(y1*w+x0)*4+j]*(1.0-fx) + src.d[(y1*w+x1)*4+j]*fx;
            o.d[(y*w+x)*4+j] = a*(1.0-fy) + b*fy;
        }
    }}
    o
}

fn blend(dst: &mut Frame, src: &Frame, opacity: f32, mode: &str, mask: Option<&dyn Fn(usize, usize) -> f32>) {
    for y in 0..dst.h { for x in 0..dst.w { let i = (y*dst.w+x)*4;
        let m = mask.map(|m| m(x, y)).unwrap_or(1.0);
        let a = src.d[i+3] * opacity * m; if a <= 0.0 { continue; }
        for j in 0..3 {
            let (b, s) = (dst.d[i+j], src.d[i+j]);
            let r = match mode { "add" => b + s, "multiply" => b*s, "screen" => 1.0-(1.0-b)*(1.0-s), "overlay" => if b<0.5 {2.0*b*s} else {1.0-2.0*(1.0-b)*(1.0-s)}, _ => s };
            dst.d[i+j] = b*(1.0-a) + r*a;
        }
        dst.d[i+3] = dst.d[i+3] + a*(1.0-dst.d[i+3]);
    }}
}

pub fn render_frame(p: &Project, t: f64, cache: &Cache) -> Result<Frame, String> {
    let (w, h) = (p.width as usize, p.height as usize);
    let mut out = Frame::new(w, h);
    for i in 0..w*h { out.d[i*4+3] = 1.0; } // opaque black canvas
    let seed = (t * p.fps).round() as u64;
    for tr in &p.tracks { for c in &tr.clips {
        if t < c.start || t >= c.start + c.duration { continue; }
        let lt = t - c.start;
        if let Source::Adjustment = c.source {
            let mut adj = Frame { w, h, d: out.d.clone() };
            for e in &c.effects { effects::apply(&e.kind, &e.params, &e.strings, &mut adj, lt, seed)?; }
            let op = prop(c, "opacity", lt) as f32; let mk: Option<Frame> = c.mask.as_ref().map(|m| { let mut mf = Frame::new(w, h); for i in 0..w*h { mf.d[i*4+3] = 1.0; } apply_mask(&mut mf, m, lt); mf });
            for i in 0..w*h { let k = op * mk.as_ref().map(|m| m.d[i*4+3]).unwrap_or(1.0); for j in 0..3 { out.d[i*4+j] = out.d[i*4+j] * (1.0 - k) + adj.d[i*4+j] * k; } }
            continue;
        }
        let mut f = source_frame(p, c, lt, cache);
        for e in &c.effects { effects::apply(&e.kind, &e.params, &e.strings, &mut f, lt, seed)?; }
        if let Some(m) = &c.mask { apply_mask(&mut f, m, lt); }
        let mut op = prop(c, "opacity", lt) as f32;
        let (mut dx, mut dy) = (prop(c, "x", lt), prop(c, "y", lt));
        let mut mask: Option<Box<dyn Fn(usize, usize) -> f32>> = None;
        if let Some(tr_in) = &c.transition_in { if lt < tr_in.duration {
            let k = (lt / tr_in.duration) as f32;
            match tr_in.kind.as_str() {
                "crossfade" | "dissolve" => op *= k,
                "wipe_left" => { let edge = k * w as f32; mask = Some(Box::new(move |x, _| if (x as f32) < edge { 1.0 } else { 0.0 })); }
                "wipe_up" => { let edge = k * h as f32; mask = Some(Box::new(move |_, y| if (y as f32) < edge { 1.0 } else { 0.0 })); }
                "iris" => { let r = k * (w as f32 * 0.8); let (cx, cy) = (w as f32/2.0, h as f32/2.0); mask = Some(Box::new(move |x, y| { let d = ((x as f32-cx).powi(2)+(y as f32-cy).powi(2)).sqrt(); ((r - d)/6.0 + 0.5).clamp(0.0, 1.0) })); }
                "wipe_right" => { let edge = (1.0 - k) * w as f32; mask = Some(Box::new(move |x, _| if (x as f32) >= edge { 1.0 } else { 0.0 })); }
                "wipe_down" => { let edge = (1.0 - k) * h as f32; mask = Some(Box::new(move |_, y| if (y as f32) >= edge { 1.0 } else { 0.0 })); }
                "diagonal" => { let e = k * (w + h) as f32; mask = Some(Box::new(move |x, y| if (x + y) as f32 <= e { 1.0 } else { 0.0 })); }
                "blinds" => { let hh = (h as f32 / 10.0).max(1.0); mask = Some(Box::new(move |_, y| if (y as f32 % hh) / hh < k { 1.0 } else { 0.0 })); }
                "checker" => { let s = (w as f32 / 12.0).max(1.0); mask = Some(Box::new(move |x, y| { let c = ((x as f32 / s) as i32 + (y as f32 / s) as i32) % 2; let th = if c == 0 { k * 2.0 } else { k * 2.0 - 1.0 }; if th > 0.5 { 1.0 } else { 0.0 } })); }
                "slide_right" => dx -= (1.0 - k as f64) * w as f64,
                "slide_down" => dy -= (1.0 - k as f64) * h as f64,
                "spin" => { op *= k; }
                "flash" => { op = if k < 0.5 { 0.0 } else { op }; }
                "slide_left" => dx += (1.0 - k as f64) * w as f64,
                "slide_up" => dy += (1.0 - k as f64) * h as f64,
                "zoom" => op *= k,
                other => return Err(format!("unknown transition '{other}'")),
            }
        }}
        let mut sc = prop(c, "scale", lt);
        if let Some(tr_in) = &c.transition_in { if tr_in.kind == "zoom" && lt < tr_in.duration { sc *= 0.6 + 0.4 * (lt / tr_in.duration); } }
        let mut rot_extra = 0.0; if let Some(tr_in) = &c.transition_in { if tr_in.kind == "spin" && lt < tr_in.duration { let k = lt / tr_in.duration; rot_extra = (1.0 - k) * 180.0; sc *= 0.5 + 0.5 * k; } }
        let f = if let Some(cs) = &c.corners { corner_pin(&f, cs) } else { f };
        let f = transform(&f, sc, prop(c, "rotation", lt) + rot_extra, dx, dy);
        blend(&mut out, &f, op, &c.blend, mask.as_deref());
    }}
    Ok(out)
}

pub fn to_rgba8(f: &Frame) -> Vec<u8> { f.d.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect() }

pub fn render_png(p: &Project, t: f64, path: &str, cache: &mut Cache) -> Result<(), String> {
    prepare_at(p, cache, Some(t))?;
    let f = render_frame(p, t, cache)?;
    let mut ch = Command::new("ffmpeg").args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s",&format!("{}x{}", p.width, p.height),"-i","-","-frames:v","1",path])
        .stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    ch.stdin.take().unwrap().write_all(&to_rgba8(&f)).map_err(|e| e.to_string())?; ch.wait().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn export(p: &Project, path: &str, crf: u32) -> Result<f64, String> {
    let mut cache = Cache::new();
    let n = (p.duration() * p.fps).round() as usize;
    let mut ch = Command::new("ffmpeg").args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s",&format!("{}x{}", p.width, p.height),"-r",&format!("{}", p.fps),"-i","-","-c:v","libx264","-pix_fmt","yuv420p","-crf",&crf.to_string(),path])
        .stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let mut si = ch.stdin.take().unwrap();
    let nt = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(2).max(1);
    let mut i = 0;
    while i < n {
        let end = (i + nt * 2).min(n);
        prepare_range(p, &mut cache, i as f64 / p.fps, end as f64 / p.fps)?;
        let frames: Vec<Result<Vec<u8>, String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = (i..end).map(|k| { let cache = &cache; sc.spawn(move || render_frame(p, k as f64 / p.fps, cache).map(|f| to_rgba8(&f))) }).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect() });
        for f in frames { si.write_all(&f?).map_err(|e| e.to_string())?; }
        i = end;
    }
    drop(si); ch.wait().map_err(|e| e.to_string())?;
    // mux mixed audio
    let pcm = crate::audio::mix(p);
    if pcm.iter().any(|v| v.abs() > 1e-6) {
        let wav = format!("{path}.tmp.wav"); let vid = format!("{path}.tmp.mp4"); crate::audio::write_wav(&wav, &pcm)?; std::fs::rename(path, &vid).map_err(|e| e.to_string())?;
        let st = Command::new("ffmpeg").args(["-v","error","-y","-i",&vid,"-i",&wav,"-c:v","copy","-c:a","aac","-b:a","192k","-shortest",path]).status().map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&wav); let _ = std::fs::remove_file(&vid); if !st.success() { return Err("audio mux failed".into()); }
    }
    Ok(n as f64 / p.fps)
}

fn apply_mask(f: &mut Frame, m: &Mask, t: f64) {
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
