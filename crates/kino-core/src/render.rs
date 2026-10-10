use crate::effects::{self, Frame};
use crate::model::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// Decoded frames for media clips, keyed by clip id. Frames are RGBA8 at project size, project fps,
/// with speed already applied.
pub struct Cache { frames: HashMap<String, Vec<Vec<u8>>> }
impl Cache { pub fn new() -> Self { Cache { frames: HashMap::new() } } }

pub fn probe(path: &str) -> Result<(u32, u32, f64, f64), String> {
    let out = Command::new("ffprobe").args(["-v","error","-select_streams","v:0","-show_entries","stream=width,height,r_frame_rate,duration","-of","csv=p=0",path])
        .output().map_err(|e| format!("ffprobe: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout); let s = s.trim();
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() < 3 { return Err(format!("cannot probe {path}: {s}")); }
    let fr: Vec<f64> = parts[2].split('/').filter_map(|x| x.parse().ok()).collect();
    let fps = if fr.len() == 2 && fr[1] > 0.0 { fr[0]/fr[1] } else { 30.0 };
    Ok((parts[0].parse().unwrap_or(0), parts[1].parse().unwrap_or(0), parts.get(3).and_then(|d| d.parse().ok()).unwrap_or(0.0), fps))
}

fn decode(p: &Project, c: &Clip, path: &str) -> Result<Vec<Vec<u8>>, String> {
    let (w, h) = (p.width, p.height);
    let (speed, span) = if let Some(_) = c.speed_curve { (1.0, c.source_time(c.duration)) } else { (c.speed, c.duration * c.speed) };
    let vf = format!("setpts=PTS/{},fps={},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black@0,format=rgba", speed.max(0.01), p.fps);
    let mut child = Command::new("ffmpeg").args(["-v","error","-ss",&format!("{}", c.in_point),"-t",&format!("{}", span),"-i",path,"-vf",&vf,"-f","rawvideo","-pix_fmt","rgba","-"])
        .stdout(Stdio::piped()).spawn().map_err(|e| format!("ffmpeg: {e}"))?;
    let mut buf = Vec::new(); child.stdout.take().unwrap().read_to_end(&mut buf).map_err(|e| e.to_string())?; let _ = child.wait();
    let fs = (w*h*4) as usize;
    Ok(buf.chunks_exact(fs).map(|c| c.to_vec()).collect())
}

pub fn prepare(p: &Project, cache: &mut Cache) -> Result<(), String> {
    for t in &p.tracks { for c in &t.clips {
        if let Source::Media { asset } = &c.source {
            if cache.frames.contains_key(&c.id) { continue; }
            let a = p.assets.get(asset).ok_or(format!("asset {asset} missing"))?;
            cache.frames.insert(c.id.clone(), decode(p, c, &a.path)?);
        }
    }}
    Ok(())
}

fn source_frame(p: &Project, c: &Clip, lt: f64, cache: &Cache) -> Frame {
    let (w, h) = (p.width as usize, p.height as usize);
    let mut f = Frame::new(w, h);
    match &c.source {
        Source::Solid { color } => for i in 0..w*h { f.d[i*4]=color[0] as f32; f.d[i*4+1]=color[1] as f32; f.d[i*4+2]=color[2] as f32; f.d[i*4+3]=1.0; },
        Source::Gradient { c1, c2 } => for y in 0..h { let k = y as f64 / h as f64; for x in 0..w { let i=(y*w+x)*4; for j in 0..3 { f.d[i+j] = (c1[j]*(1.0-k)+c2[j]*k) as f32; } f.d[i+3]=1.0; } },
        Source::Text { text, font, size, color, letter_spacing, stroke_width, stroke_color, shadow, align } => {
            let _ = crate::text::render_text(&mut f, text, font, *size, *color, *letter_spacing, *stroke_width, *stroke_color, *shadow, align, prop(c, "reveal", lt));
        }
        Source::Media { .. } => if let Some(fr) = cache.frames.get(&c.id) {
            let idx = if c.speed_curve.is_some() { (c.source_time(lt) * p.fps) as usize } else { (lt * p.fps) as usize };
            let idx = idx.min(fr.len().saturating_sub(1));
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
                "slide_left" => dx += (1.0 - k as f64) * w as f64,
                "slide_up" => dy += (1.0 - k as f64) * h as f64,
                "zoom" => op *= k,
                other => return Err(format!("unknown transition '{other}'")),
            }
        }}
        let mut sc = prop(c, "scale", lt);
        if let Some(tr_in) = &c.transition_in { if tr_in.kind == "zoom" && lt < tr_in.duration { sc *= 0.6 + 0.4 * (lt / tr_in.duration); } }
        let f = transform(&f, sc, prop(c, "rotation", lt), dx, dy);
        blend(&mut out, &f, op, &c.blend, mask.as_deref());
    }}
    Ok(out)
}

pub fn to_rgba8(f: &Frame) -> Vec<u8> { f.d.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect() }

pub fn render_png(p: &Project, t: f64, path: &str) -> Result<(), String> {
    let mut cache = Cache::new(); prepare(p, &mut cache)?;
    let f = render_frame(p, t, &cache)?;
    let mut ch = Command::new("ffmpeg").args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s",&format!("{}x{}", p.width, p.height),"-i","-","-frames:v","1",path])
        .stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    ch.stdin.take().unwrap().write_all(&to_rgba8(&f)).map_err(|e| e.to_string())?; ch.wait().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn export(p: &Project, path: &str, crf: u32) -> Result<f64, String> {
    let mut cache = Cache::new(); prepare(p, &mut cache)?;
    let n = (p.duration() * p.fps).round() as usize;
    let mut ch = Command::new("ffmpeg").args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s",&format!("{}x{}", p.width, p.height),"-r",&format!("{}", p.fps),"-i","-","-c:v","libx264","-pix_fmt","yuv420p","-crf",&crf.to_string(),path])
        .stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let mut si = ch.stdin.take().unwrap();
    let nt = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(2).max(1);
    let mut i = 0;
    while i < n {
        let end = (i + nt * 2).min(n);
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
