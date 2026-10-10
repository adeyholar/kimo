//! Color tools: scope images (waveform, RGB parade, vectorscope, histogram), white-balance picker, color match.
use crate::commands::Command;
use crate::effects::Frame;
use crate::keyframe::Prop;
use crate::model::*;
use crate::render;
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command as Proc, Stdio};

fn s(d: &str) -> Value { json!({"type":"string","description":d}) }
fn n(d: &str) -> Value { json!({"type":"number","description":d}) }
fn obj(props: Vec<(&str, Value)>, req: &[&str]) -> Value { let mut m = serde_json::Map::new(); for (k, v) in props { m.insert(k.into(), v); } json!({"type":"object","properties":m,"required":req}) }
fn sa<'a>(a: &'a Value, k: &str) -> Result<&'a str, String> { a.get(k).and_then(|v| v.as_str()).ok_or(format!("missing string '{k}'")) }
fn na(a: &Value, k: &str) -> Result<f64, String> { a.get(k).and_then(|v| v.as_f64()).ok_or(format!("missing number '{k}'")) }
fn on(a: &Value, k: &str, d: f64) -> f64 { a.get(k).and_then(|v| v.as_f64()).unwrap_or(d) }

/// Frame of one clip in isolation (its own effects included) at a timeline time.
fn clip_frame(p: &Project, clip_id: &str, t: f64) -> Result<Frame, String> {
    let mut q = p.clone(); let mut keep = None; for (ti, tr) in q.tracks.iter().enumerate() { if tr.clips.iter().any(|c| c.id == clip_id) { keep = Some(ti); } }
    let ti = keep.ok_or(format!("no clip '{clip_id}'"))?; let mut tr = q.tracks[ti].clone(); tr.clips.retain(|c| c.id == clip_id); q.tracks = vec![tr];
    let mut cache = render::Cache::new(); render::prepare_at(&q, &mut cache, Some(t))?; render::render_frame(&q, t, &cache)
}
fn any_frame(p: &Project, clip: Option<&str>, t: f64) -> Result<Frame, String> { match clip { Some(c) => clip_frame(p, c, t), None => { let mut cache = render::Cache::new(); render::prepare_at(p, &mut cache, Some(t))?; render::render_frame(p, t, &cache) } } }
fn stats(f: &Frame, region: Option<[f64; 4]>) -> ([f32; 3], [f32; 3]) {
    let (x0, y0, x1, y1) = match region { Some(r) => ((r[0] * f.w as f64) as usize, (r[1] * f.h as f64) as usize, ((r[0] + r[2]) * f.w as f64) as usize, ((r[1] + r[3]) * f.h as f64) as usize), None => (0, 0, f.w, f.h) };
    let (mut s1, mut s2, mut c) = ([0f64; 3], [0f64; 3], 0f64); for y in y0..y1.min(f.h) { for x in x0..x1.min(f.w) { for j in 0..3 { let v = f.d[(y * f.w + x) * 4 + j].clamp(0.0, 1.0) as f64; s1[j] += v; s2[j] += v * v; } c += 1.0; } }
    let c = c.max(1.0); let m = [(s1[0] / c) as f32, (s1[1] / c) as f32, (s1[2] / c) as f32]; let sd = [0, 1, 2].map(|j| ((s2[j] / c - (s1[j] / c).powi(2)).max(0.0).sqrt()) as f32); (m, sd) }

fn add_fx(p: &mut Project, clip_id: &str, kind: &str, vals: &[(&str, f64)]) -> Result<String, String> {
    let id = p.gen_id("fx"); let c = p.find_clip_mut(clip_id).ok_or("no such clip")?; let mut params = std::collections::BTreeMap::new(); for (k, v) in vals { params.insert(k.to_string(), Prop::new(*v)); }
    c.effects.push(Effect { id: id.clone(), kind: kind.into(), params, strings: Default::default() }); Ok(id) }

/// Render a scope to a PNG: waveform | parade | vectorscope | histogram.
pub fn scope_png(f: &Frame, kind: &str, path: &str) -> Result<(), String> {
    let (ow, oh) = match kind { "vectorscope" => (320usize, 320usize), _ => (512, 288) }; let mut acc = vec![0f32; ow * oh * 3];
    let lum = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let step = ((f.w * f.h) / 200_000).max(1);
    match kind {
        "waveform" | "parade" => { let parade = kind == "parade"; for y in (0..f.h).step_by(1) { for x in (0..f.w).step_by(step.min(f.w)) { let i = (y * f.w + x) * 4; let c = [f.d[i], f.d[i + 1], f.d[i + 2]];
            if parade { let third = ow / 3; for j in 0..3 { let ox = j * third + x * third / f.w; let oy = ((1.0 - c[j].clamp(0.0, 1.0)) * (oh - 1) as f32) as usize; acc[(oy * ow + ox) * 3 + j] += 1.0; } }
            else { let ox = x * ow / f.w; let l = lum(c).clamp(0.0, 1.0); let oy = ((1.0 - l) * (oh - 1) as f32) as usize; for j in 0..3 { acc[(oy * ow + ox) * 3 + j] += 1.0; } } } } }
        "vectorscope" => { for y in (0..f.h).step_by(1) { for x in (0..f.w).step_by(step.min(f.w)) { let i = (y * f.w + x) * 4; let (r, g, b) = (f.d[i].clamp(0.0, 1.0), f.d[i + 1].clamp(0.0, 1.0), f.d[i + 2].clamp(0.0, 1.0)); let yy = 0.2126 * r + 0.7152 * g + 0.0722 * b; let cb = (b - yy) / 1.8556; let cr = (r - yy) / 1.5748;
            let ox = ((cb * 1.0 + 0.5) * (ow - 1) as f32) as usize; let oy = ((0.5 - cr * 1.0) * (oh - 1) as f32) as usize; let k = (oy.min(oh - 1) * ow + ox.min(ow - 1)) * 3; acc[k] += r; acc[k + 1] += g; acc[k + 2] += b; } } }
        "histogram" => { let mut hist = vec![[0f32; 3]; ow]; for i in (0..f.w * f.h).step_by(step) { for j in 0..3 { let v = f.d[i * 4 + j].clamp(0.0, 1.0); hist[((v * (ow - 1) as f32) as usize).min(ow - 1)][j] += 1.0; } }
            let mx = hist.iter().flat_map(|h| h.iter()).cloned().fold(1.0, f32::max).sqrt(); for x in 0..ow { for j in 0..3 { let hh = ((hist[x][j].sqrt() / mx) * oh as f32) as usize; for y in oh.saturating_sub(hh)..oh { acc[(y * ow + x) * 3 + j] += 1.0; } } } }
        o => return Err(format!("unknown scope '{o}' (waveform|parade|vectorscope|histogram)")) }
    let mx = acc.iter().cloned().fold(1.0, f32::max); let gain = if kind == "histogram" { 0.55 } else { 14.0 / (mx.sqrt().max(1.0)) };
    let mut out = vec![0u8; ow * oh * 4];
    for y in 0..oh { for x in 0..ow { let k = (y * ow + x) * 3; let o = (y * ow + x) * 4; let a = [acc[k], acc[k + 1], acc[k + 2]]; let mut c = if kind == "waveform" || kind == "vectorscope" { let m = (a[0] + a[1] + a[2]) / 3.0; let t = (m.max(0.0).sqrt() * gain / 4.0).min(1.0); if kind == "waveform" { [0.2 * t + 0.0, 1.0 * t, 0.35 * t] } else { let (mr, mgg, mb) = if m > 0.0 { (a[0] / m, a[1] / m, a[2] / m) } else { (0.0, 0.0, 0.0) }; [(mr * t).min(1.0) * 1.0, (mgg * t).min(1.0), (mb * t).min(1.0)] } } else { [(a[0].sqrt() * gain).min(1.0), (a[1].sqrt() * gain).min(1.0), (a[2].sqrt() * gain).min(1.0)] };
        if kind == "parade" || kind == "histogram" { let _ = &mut c; }
        // graticule
        let (gx, gy) = (x as f32 / ow as f32, y as f32 / oh as f32);
        if kind == "waveform" || kind == "parade" { for lv in [0.0, 0.1, 0.5, 0.9, 1.0] { if ((1.0 - gy) - lv).abs() < 0.0018 { for j in 0..3 { c[j] = c[j].max(0.22); } } } }
        if kind == "vectorscope" { let (dx, dy) = (gx - 0.5, gy - 0.5); let r = (dx * dx + dy * dy).sqrt(); for rr in [0.25, 0.5] { if (r - rr * 0.5 * 1.0).abs() < 0.002 { for j in 0..3 { c[j] = c[j].max(0.25); } } } let ang = (-dy).atan2(dx).to_degrees(); if ((ang - 123.0).abs() < 0.6 && r < 0.5) { c[0] = c[0].max(0.9); c[1] = c[1].max(0.6); c[2] = c[2].max(0.4); } if dx.abs() < 0.002 || dy.abs() < 0.002 { for j in 0..3 { c[j] = c[j].max(0.15); } } }
        for j in 0..3 { out[o + j] = (c[j].clamp(0.0, 1.0) * 255.0) as u8; } out[o + 3] = 255; } }
    let mut ch = Proc::new("ffmpeg").args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{ow}x{oh}"), "-i", "-", "-frames:v", "1", path]).stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    ch.stdin.take().unwrap().write_all(&out).map_err(|e| e.to_string())?; ch.wait().map_err(|e| e.to_string())?; Ok(()) }

pub fn commands() -> Vec<Command> { vec![
    Command { name: "render_scope", description: "Render a video scope image of the frame (optionally one clip in isolation): waveform (luma), parade (RGB), vectorscope (Cb/Cr, BT.709, skin-tone line), histogram. Look at it to judge exposure, color casts and clipping.", mutates: false,
        schema: obj(vec![("time", n("timeline seconds")), ("scope", json!({"type":"string","enum":["waveform","parade","vectorscope","histogram"]})), ("path", s("output png")), ("clip_id", s("optional: scope this clip only"))], &["time","scope","path"]),
        run: |ss, a| { let f = any_frame(&ss.project, a.get("clip_id").and_then(|v| v.as_str()), na(a, "time")?)?; scope_png(&f, sa(a, "scope")?, sa(a, "path")?)?; Ok(json!({"path": sa(a, "path")?})) } },
    Command { name: "pick_white_balance", description: "White balance picker: sample a region (x,y,w,h as 0..1 fractions) that should be neutral grey/white and add a white_balance effect with the correcting gains to the clip. Samples the clip before its other effects are applied when it has none after; returns the gains.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("time", n("timeline seconds")), ("x", n("0..1")), ("y", n("0..1")), ("w", n("0..1")), ("h", n("0..1"))], &["clip_id","time"]),
        run: |ss, a| { let id = sa(a, "clip_id")?; let t = na(a, "time")?; let f = clip_frame(&ss.project, id, t)?; let (m, _) = stats(&f, Some([on(a, "x", 0.4), on(a, "y", 0.4), on(a, "w", 0.2), on(a, "h", 0.2)]));
            let tgt = (m[0] + m[1] + m[2]) / 3.0; let g = [tgt / m[0].max(0.01), tgt / m[1].max(0.01), tgt / m[2].max(0.01)]; let fx = add_fx(&mut ss.project, id, "white_balance", &[("r_gain", g[0] as f64), ("g_gain", g[1] as f64), ("b_gain", g[2] as f64)])?; Ok(json!({"effect_id": fx, "gains": g, "sampled_rgb": m})) } },
    Command { name: "match_color", description: "Color match: make clip_id look like ref_clip_id by matching per-channel mean and contrast (Reinhard transfer). Adds a color_transfer effect; strength 0..1.", mutates: true,
        schema: obj(vec![("clip_id", s("clip to grade")), ("ref_clip_id", s("clip to match")), ("time", n("timeline time inside clip_id")), ("ref_time", n("timeline time inside ref clip")), ("strength", n("0..1"))], &["clip_id","ref_clip_id","time","ref_time"]),
        run: |ss, a| { let (id, rid) = (sa(a, "clip_id")?, sa(a, "ref_clip_id")?); let fs = clip_frame(&ss.project, id, na(a, "time")?)?; let fr = clip_frame(&ss.project, rid, na(a, "ref_time")?)?; let (sm, sd) = stats(&fs, None); let (tm, td) = stats(&fr, None);
            let v: Vec<(&str, f64)> = vec![("sr_mean", sm[0] as f64), ("sg_mean", sm[1] as f64), ("sb_mean", sm[2] as f64), ("sr_std", sd[0] as f64), ("sg_std", sd[1] as f64), ("sb_std", sd[2] as f64), ("tr_mean", tm[0] as f64), ("tg_mean", tm[1] as f64), ("tb_mean", tm[2] as f64), ("tr_std", td[0] as f64), ("tg_std", td[1] as f64), ("tb_std", td[2] as f64), ("strength", on(a, "strength", 1.0))];
            let fx = add_fx(&mut ss.project, id, "color_transfer", &v)?; Ok(json!({"effect_id": fx, "src_mean": sm, "ref_mean": tm})) } },
] }
