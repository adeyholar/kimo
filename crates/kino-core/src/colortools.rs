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
    let ti = keep.ok_or(format!("no clip '{clip_id}'"))?; let mut tr = q.tracks[ti].clone(); tr.clips.retain(|c| c.id == clip_id);
    let c = &mut tr.clips[0]; let local = t - c.start;
    if !local.is_finite() || local < -1e-7 || local >= c.duration { return Err("sample time must be inside clip".into()); }
    // Snap floating error from shifted frame boundaries, retaining the original
    // timeline clock for dynamic text/expressions and time-dependent source content.
    let frame = local.max(0.0) * q.fps;
    let snapped = if (frame-frame.round()).abs() < 1e-7 { c.start + frame.round()/q.fps + 1e-8/q.fps } else { t };
    q.tracks = vec![tr];
    let mut cache = render::Cache::new(); render::prepare_at(&q, &mut cache, Some(snapped))?; render::render_frame(&q, snapped, &cache)
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
    Command { name:"export_grade_lut",description:"Bake one clip's supported pointwise color effects at a frozen timeline time into a .cube LUT (17/33/65 grid). Rejects spatial/unsupported effects rather than silently omitting them. Does not include masks, transforms or footage. Domain default 0..1.",mutates:false,
        schema:obj(vec![("clip_id",s("clip")),("time",n("freeze parameter time")),("path",s("output .cube")),("size",n("17,33 or 65; default 33")),("domain_min",n("default 0")),("domain_max",n("default 1"))],&["clip_id","time","path"]),
        run:|ss,a|{
            let size=on(a,"size",33.);if ![17.,33.,65.].contains(&size){return Err("LUT size must be 17,33,65".into())}let n=size as usize;let lo=on(a,"domain_min",0.);let hi=on(a,"domain_max",1.);if !lo.is_finite()||!hi.is_finite()||lo>=hi||lo.abs()>16.||hi.abs()>16.{return Err("finite increasing LUT domain within +/-16 required".into())}
            let c=ss.project.find_clip(sa(a,"clip_id")?).ok_or("no clip")?;
            let allowed=["linear_grade","color_managed","point_curves","lab_match","cdl","color_space","primaries","log_wheels","rgb_curves","hue_curves","film_print","bleach_bypass","white_balance","color_transfer","tonemap","highlight_rolloff","split_toning","exposure","contrast","saturation","temperature","tint","color_wheels","curves","teal_orange","levels","brightness_contrast","channel_mixer","colorize","invert","sepia","lut","hsl"];
            for e in &c.effects{if !allowed.contains(&e.kind.as_str())||e.strings.get("mask_layer").map(|v|!v.is_empty()).unwrap_or(false){return Err(format!("cannot bake spatial/masked/unsupported effect '{}'",e.kind))}}
            let mut f=Frame::new(n*n,n);for b in 0..n{for g in 0..n{for r in 0..n{let i=((b*n+g)*n+r)*4;for (j,k) in [r,g,b].iter().enumerate(){f.d[i+j]=(lo+(hi-lo)* *k as f64/(n-1) as f64) as f32}f.d[i+3]=1.}}}
            let t=na(a,"time")?-c.start;for e in &c.effects{crate::effects::apply(&e.kind,&e.params,&e.strings,&mut f,t,0)?}
            if f.d.iter().any(|v|!v.is_finite()){return Err("grade produces nonfinite LUT values".into())}
            let path=sa(a,"path")?;let file=std::fs::File::create(path).map_err(|e|e.to_string())?;let mut w=std::io::BufWriter::new(file);
            writeln!(w,"TITLE \"Kino pointwise grade\"\nLUT_3D_SIZE {n}\nDOMAIN_MIN {lo} {lo} {lo}\nDOMAIN_MAX {hi} {hi} {hi}").map_err(|e|e.to_string())?;
            for px in f.d.chunks_exact(4){writeln!(w,"{:.8} {:.8} {:.8}",px[0],px[1],px[2]).map_err(|e|e.to_string())?}w.flush().map_err(|e|e.to_string())?;
            Ok(json!({"path":path,"size":n,"entries":n*n*n,"effects":c.effects.len(),"domain":[lo,hi],"frozen_time":t}))
        } },
    Command { name:"match_grade",description:"Perceptual sRGB/D65 shot grade matching: full Lab covariance optimal transport rather than independent RGB mean/std. Optional normalized sample regions; excludes transparent pixels. Adds lab_match. Not semantic matching or camera calibration.",mutates:true,
        schema:obj(vec![("clip_id",s("source clip")),("ref_clip_id",s("reference clip")),("time",n("source timeline seconds")),("ref_time",n("reference timeline seconds")),("strength",n("0..1")),("regularization",n("1e-8..0.1, default 0.0001")),("source_region",json!({"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4})),("ref_region",json!({"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4}))],&["clip_id","ref_clip_id","time","ref_time"]),
        run:|ss,a|{let id=sa(a,"clip_id")?;let rid=sa(a,"ref_clip_id")?;if id==rid{return Err("source and reference must differ".into())}
            let region=|key:&str|->Result<Option<[f64;4]>,String>{match a.get(key){None=>Ok(None),Some(v)=>serde_json::from_value(v.clone()).map(Some).map_err(|e|e.to_string())}};
            let fs=clip_frame(&ss.project,id,na(a,"time")?)?;let fr=clip_frame(&ss.project,rid,na(a,"ref_time")?)?;
            let src=crate::color::matching::statistics(&fs,region("source_region")?)?;let dst=crate::color::matching::statistics(&fr,region("ref_region")?)?;let m=crate::color::matching::transport(&src,&dst,on(a,"regularization",0.0001))?;
            let strength=on(a,"strength",1.);if !strength.is_finite()||!(0. ..=1.).contains(&strength){return Err("strength outside 0..1".into())}
            let mut vals=Vec::new();for i in 0..3{vals.push((format!("s{i}"),src.mean[i]));vals.push((format!("t{i}"),dst.mean[i]));for j in 0..3{vals.push((format!("a{i}{j}"),m[i][j]));}}vals.push(("strength".into(),strength));
            let v:Vec<_>=vals.iter().map(|(k,v)|(k.as_str(),*v)).collect();let fx=add_fx(&mut ss.project,id,"lab_match",&v)?;Ok(json!({"effect_id":fx,"source_samples":src.count,"reference_samples":dst.count,"source_lab_mean":src.mean,"reference_lab_mean":dst.mean,"transport":m,"assumption":"sRGB encoded / D65"}))} },
    Command { name: "analyze_color", description: "Quantitative float RGB/luma histograms, waveform and BT709 CbCr density; un-clipped min/max/mean and under/over-range counts. No nits inference. Optionally isolate one clip.", mutates:false,
        schema:obj(vec![("time",n("timeline seconds")),("clip_id",s("optional isolated clip")),("bins",n("16..4096 default 256")),("range_min",n("default 0")),("range_max",n("default 1"))],&["time"]),
        run:|ss,a|{let bins=on(a,"bins",256.);if !bins.is_finite()||bins.fract()!=0.||!(16. ..=4096.).contains(&bins){return Err("invalid bins".into())}let f=any_frame(&ss.project,a.get("clip_id").and_then(Value::as_str),na(a,"time")?)?;crate::color::scopes::data(&f,bins as usize,on(a,"range_min",0.),on(a,"range_max",1.))} },
    Command { name: "set_color_curve", description: "Add a free-position master/RGB PCHIP color curve. points is 2..32 increasing [x,y] pairs, channel m/r/g/b. Other channels remain identity. Extended values extrapolate linearly.", mutates: true,
        schema: obj(vec![("clip_id",s("clip")),("channel",json!({"type":"string","enum":["m","r","g","b"]})),("points",json!({"type":"array","minItems":2,"maxItems":32,"items":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}}}))], &["clip_id","channel","points"]),
        run: |ss,a| { let ch=sa(a,"channel")?;if !["m","r","g","b"].contains(&ch){return Err("invalid curve channel".into())}
            let pts:Vec<[f32;2]>=serde_json::from_value(a.get("points").ok_or("missing points")?.clone()).map_err(|e|e.to_string())?;
            crate::color::curves::Curve::new(&pts)?;
            let id=sa(a,"clip_id")?;if ss.project.find_clip(id).is_none(){return Err("no clip".into())}
            let fx=ss.project.gen_id("fx");let mut params=std::collections::BTreeMap::new();params.insert(format!("{ch}_count"),Prop::new(pts.len() as f64));
            for (i,pt) in pts.iter().enumerate(){params.insert(format!("{ch}_x{i}"),Prop::new(pt[0] as f64));params.insert(format!("{ch}_y{i}"),Prop::new(pt[1] as f64));}
            ss.project.find_clip_mut(id).unwrap().effects.push(Effect{id:fx.clone(),kind:"point_curves".into(),params,strings:Default::default()});Ok(json!({"effect_id":fx,"points":pts.len()})) } },
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

#[cfg(test)] mod color_sampling_tests {
    use super::*;
    #[test] fn shifted_clip_samples_same_keyframed_frame() {
        use crate::commands::{dispatch, Session};
        let mut ss=Session::new();dispatch(&mut ss,"project_set",&json!({"width":64,"height":64,"fps":25})).unwrap();
        let tr=dispatch(&mut ss,"add_track",&json!({})).unwrap()["track_id"].as_str().unwrap().to_string();
        let a=dispatch(&mut ss,"add_clip",&json!({"track_id":tr,"source_type":"solid","color":[0.3,0.4,0.5],"start":3,"duration":2})).unwrap()["clip_id"].as_str().unwrap().to_string();
        dispatch(&mut ss,"set_keyframe",&json!({"clip_id":a,"property":"opacity","time":0.8,"value":0.25})).unwrap();
        let f=clip_frame(&ss.project,&a,3.8).unwrap();
        let mut zero=ss.project.clone();zero.find_clip_mut(&a).unwrap().start=0.;let z=clip_frame(&zero,&a,0.8).unwrap();
        for (a,b) in f.d.iter().zip(z.d.iter()) { assert!((a-b).abs()<1e-7); }
        assert!(clip_frame(&ss.project,&a,2.).is_err());assert!(clip_frame(&ss.project,&a,5.).is_err());
    }
}
