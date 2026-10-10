//! HUD / info-graphic one-call tools: detection boxes with label tags, number counters, typing prompt boxes.
use crate::commands::Session;
use crate::keyframe::{Ease, Prop};
use crate::model::*;
use serde_json::{json, Value};

fn clip(ss: &mut Session, source: Source, start: f64, dur: f64) -> Clip { Clip { id: ss.project.gen_id("clip"), source, start, duration: dur, in_point: 0.0, speed: 1.0, speed_curve: None, props: default_props(), effects: vec![], blend: "normal".into(), mask: None, time_mode: TimeMode::Normal, corners: None, audio: Audio::default(), motion_blur: 0.0, text_fx: None, transition_in: None } }
fn num(a: &Value, k: &str, d: f64) -> f64 { a.get(k).and_then(|v| v.as_f64()).unwrap_or(d) }
fn rgb(a: &Value, k: &str, d: [f64; 3]) -> [f64; 3] { a.get(k).and_then(|v| v.as_array()).filter(|v| v.len() >= 3).map(|v| [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0), v[2].as_f64().unwrap_or(0.0)]).unwrap_or(d) }
fn txt(text: &str, font: &str, size: f64, color: [f64; 3], align: &str) -> Source { Source::Text { text: text.into(), font: font.into(), size, color, letter_spacing: 0.0, stroke_width: 0.0, stroke_color: [0.0; 3], shadow: 0.0, align: align.into() } }
fn shape(kind: &str, w: f64, h: f64, fill: [f64; 3], sw: f64, sc: [f64; 3]) -> Source { Source::Shape { shape: kind.into(), width: w, height: h, fill, stroke_width: sw, stroke_color: sc, sides: 4, corner_radius: 0.0 } }
fn set(c: &mut Clip, p: &str, v: f64) { c.props.get_mut(p).unwrap().base = v; }
fn key(c: &mut Clip, p: &str, t: f64, v: f64, e: Ease) { c.props.get_mut(p).unwrap().set_key(t, v, e); }
fn push(ss: &mut Session, tid: &str, clips: Vec<Clip>) -> Result<Vec<String>, String> { let ids = clips.iter().map(|c| c.id.clone()).collect(); ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or("no such track")?.clips.extend(clips); Ok(ids) }

/// Object-detection style overlay: hollow box + corner ticks + filled label tag with "label 0.92".
pub fn add_detection_box(ss: &mut Session, a: &Value) -> Result<Value, String> {
    let tid = a.get("track_id").and_then(|v| v.as_str()).ok_or("missing track_id")?.to_string();
    let (start, dur) = (num(a, "start", 0.0), num(a, "duration", 3.0)); if dur <= 0.0 { return Err("duration must be > 0".into()); }
    let (cw, ch) = (ss.project.width as f64, ss.project.height as f64); let k = ch / 720.0;
    let (bx, by, bw, bh) = (num(a, "x", 0.3), num(a, "y", 0.3), num(a, "w", 0.3), num(a, "h", 0.4));
    if !(bw > 0.0 && bh > 0.0) { return Err("w and h must be > 0".into()); }
    let col = rgb(a, "color", [0.2, 0.9, 0.5]); let label = a.get("label").and_then(|v| v.as_str()).unwrap_or("object").to_string(); let conf = num(a, "confidence", 0.92);
    let font = a.get("font").and_then(|v| v.as_str()).unwrap_or("SpaceMono-Bold"); let fsz = 20.0 * k; let tag = format!("{} {:.2}", label, conf);
    let (pw, ph) = (bw * cw, bh * ch); let (cx, cy) = ((bx + bw / 2.0 - 0.5) * cw, (by + bh / 2.0 - 0.5) * ch);
    let tw = tag.chars().count() as f64 * fsz * 0.6 + 16.0 * k; let th = fsz * 1.5;
    let sw = (2.0 * k).max(1.5); let fill_alpha_box = num(a, "fill_opacity", 0.08);
    let e = Ease::OutExpo; let tin = 0.35; let mut cl = vec![];
    // soft fill
    let mut f = clip(ss, shape("rect", pw, ph, col, 0.0, col), start, dur); set(&mut f, "x", cx); set(&mut f, "y", cy); f.props.get_mut("opacity").unwrap().base = 0.0; key(&mut f, "opacity", 0.0, 0.0, e); key(&mut f, "opacity", tin, fill_alpha_box, e); key(&mut f, "opacity", dur - 0.15, fill_alpha_box, Ease::In); key(&mut f, "opacity", dur, 0.0, Ease::In); cl.push(f);
    // outline
    let mut o = clip(ss, shape("rect", pw, ph, [-1.0, 0.0, 0.0], sw, col), start, dur); set(&mut o, "x", cx); set(&mut o, "y", cy);
    for (p, v0) in [("scale", 1.12)] { let pr = o.props.get_mut(p).unwrap(); pr.base = 1.0; pr.set_key(0.0, v0, e); pr.set_key(tin, 1.0, e); }
    key(&mut o, "opacity", 0.0, 0.0, e); key(&mut o, "opacity", 0.15, 1.0, e); key(&mut o, "opacity", dur - 0.15, 1.0, Ease::In); key(&mut o, "opacity", dur, 0.0, Ease::In); o.motion_blur = 180.0; cl.push(o);
    // corner ticks (thick short bars at four corners)
    let tl = (pw.min(ph) * 0.14).clamp(8.0 * k, 36.0 * k); let tk = sw * 2.2;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        for horiz in [true, false] {
            let (w, h) = if horiz { (tl, tk) } else { (tk, tl) };
            let mut t = clip(ss, shape("rect", w, h, col, 0.0, col), start, dur);
            set(&mut t, "x", cx + sx * (pw / 2.0 - if horiz { tl / 2.0 } else { tk / 2.0 })); set(&mut t, "y", cy + sy * (ph / 2.0 - if horiz { tk / 2.0 } else { tl / 2.0 }));
            key(&mut t, "opacity", 0.0, 0.0, e); key(&mut t, "opacity", 0.2, 1.0, e); key(&mut t, "opacity", dur - 0.15, 1.0, Ease::In); key(&mut t, "opacity", dur, 0.0, Ease::In); cl.push(t);
        }
    }
    // label tag: filled plate sitting on the top-left corner, then text; wipes open from the left
    let (tx, ty) = (cx - pw / 2.0 + tw / 2.0, cy - ph / 2.0 - th / 2.0 - 1.0); let d0 = 0.18;
    let mut plate = clip(ss, shape("rect", tw, th, col, 0.0, col), start + d0, dur - d0); set(&mut plate, "x", tx); set(&mut plate, "y", ty);
    key(&mut plate, "scale", 0.0, 0.0, e); key(&mut plate, "scale", 0.3, 1.0, e); key(&mut plate, "opacity", dur - d0 - 0.15, 1.0, Ease::In); key(&mut plate, "opacity", dur - d0, 0.0, Ease::In); cl.push(plate);
    let lum = 0.2126 * col[0] + 0.7152 * col[1] + 0.0722 * col[2]; let ink = if lum > 0.45 { [0.04, 0.05, 0.05] } else { [1.0; 3] };
    let mut tt = clip(ss, txt(&tag, font, fsz, ink, "center"), start + d0, dur - d0); set(&mut tt, "x", tx); set(&mut tt, "y", ty);
    key(&mut tt, "reveal", 0.05, 0.0, Ease::Linear); key(&mut tt, "reveal", 0.35, 1.0, Ease::Out); key(&mut tt, "opacity", dur - d0 - 0.15, 1.0, Ease::In); key(&mut tt, "opacity", dur - d0, 0.0, Ease::In); cl.push(tt);
    let ids = push(ss, &tid, cl)?; Ok(json!({"clip_ids": ids}))
}

/// Number counter: shows a rolling number from `from` to `to` with an ease, as a sequence of text clips (one per frame step).
pub fn add_counter(ss: &mut Session, a: &Value) -> Result<Value, String> {
    let tid = a.get("track_id").and_then(|v| v.as_str()).ok_or("missing track_id")?.to_string();
    let (start, dur, hold) = (num(a, "start", 0.0), num(a, "duration", 3.0), num(a, "hold", 1.0)); let (from, to) = (num(a, "from", 0.0), num(a, "to", 100.0));
    if dur <= 0.0 || hold < 0.0 { return Err("duration must be > 0, hold >= 0".into()); }
    let fps = ss.project.fps; let roll = dur.min(2.0).max(0.1); let n = ((roll * fps).round() as usize).clamp(2, 600);
    let (size, color) = (num(a, "size", 120.0), rgb(a, "color", [1.0; 3])); let font = a.get("font").and_then(|v| v.as_str()).unwrap_or("Anton").to_string();
    let (suffix, prefix) = (a.get("suffix").and_then(|v| v.as_str()).unwrap_or("").to_string(), a.get("prefix").and_then(|v| v.as_str()).unwrap_or("").to_string());
    let (px, py) = (num(a, "x", 0.0), num(a, "y", 0.0)); let decimals = num(a, "decimals", 0.0).clamp(0.0, 4.0) as usize; let mut cl = vec![]; let step = 1.0 / fps;
    for i in 0..n {
        let u = (i as f64 / (n - 1) as f64).clamp(0.0, 1.0); let eased = 1.0 - (1.0 - u).powi(4); let v = from + (to - from) * eased;
        let s = format!("{}{:.*}{}", prefix, decimals, v, suffix); let t0 = start + i as f64 * step; let last = i == n - 1;
        let d = if last { (start + dur + hold - t0).max(step) } else { step };
        let mut c = clip(ss, txt(&s, &font, size, color, "center"), t0, d); set(&mut c, "x", px); set(&mut c, "y", py);
        if last { key(&mut c, "scale", 0.0, 1.0, Ease::Out); key(&mut c, "scale", 0.12, 1.06, Ease::Out); key(&mut c, "scale", 0.4, 1.0, Ease::Out); }
        cl.push(c);
    }
    let ids = push(ss, &tid, cl)?; Ok(json!({"clip_ids": ids, "frames": n}))
}

/// Prompt box: rounded dark plate with accent border, typed text and a blinking caret.
pub fn add_prompt_box(ss: &mut Session, a: &Value) -> Result<Value, String> {
    let tid = a.get("track_id").and_then(|v| v.as_str()).ok_or("missing track_id")?.to_string();
    let (start, dur) = (num(a, "start", 0.0), num(a, "duration", 4.0)); let text = a.get("text").and_then(|v| v.as_str()).ok_or("missing text")?.to_string();
    let (cw, ch) = (ss.project.width as f64, ss.project.height as f64); let k = ch / 720.0; let accent = rgb(a, "accent", [0.95, 0.75, 0.3]); let type_time = num(a, "type_time", (text.chars().count() as f64 * 0.045).clamp(0.5, 3.5)).min(dur);
    let fsz = num(a, "size", 24.0) * k; let (w, h) = (cw * 0.62, fsz * 3.2); let y = num(a, "y", 0.3 * ch); let e = Ease::OutExpo; let mut cl = vec![];
    let mut bg = clip(ss, Source::Shape { shape: "rect".into(), width: w, height: h, fill: [0.07, 0.07, 0.08], stroke_width: 2.0 * k, stroke_color: accent, sides: 4, corner_radius: 10.0 * k }, start, dur); set(&mut bg, "y", y);
    key(&mut bg, "scale", 0.0, 0.9, e); key(&mut bg, "scale", 0.4, 1.0, e); key(&mut bg, "opacity", 0.0, 0.0, e); key(&mut bg, "opacity", 0.25, 0.94, e); key(&mut bg, "opacity", dur - 0.2, 0.94, Ease::In); key(&mut bg, "opacity", dur, 0.0, Ease::In); cl.push(bg);
    let mut t = clip(ss, txt(&text, a.get("font").and_then(|v| v.as_str()).unwrap_or("Inter"), fsz, [0.96, 0.95, 0.9], "left"), start + 0.3, dur - 0.3); set(&mut t, "y", y); set(&mut t, "x", -w / 2.0 + fsz * 1.2 + 0.0);
    key(&mut t, "reveal", 0.0, 0.0, Ease::Linear); key(&mut t, "reveal", type_time, 1.0, Ease::Linear); key(&mut t, "opacity", dur - 0.5, 1.0, Ease::In); key(&mut t, "opacity", dur - 0.3, 0.0, Ease::In); cl.push(t);
    // blinking caret: alternating on/off short clips after typing, plus a solid caret during typing is omitted (text engine shows reveal)
    let mut tcur = start + 0.3 + type_time; let end = start + dur - 0.3; let cx = -w / 2.0 + fsz * 1.2 + 0.0;
    let _ = cx; let mut on = true; while tcur < end - 0.01 { let d = 0.5f64.min(end - tcur); if on { let mut c = clip(ss, shape("rect", 2.5 * k, fsz * 1.1, [0.96, 0.95, 0.9], 0.0, [0.0; 3]), tcur, d); set(&mut c, "y", y); set(&mut c, "x", -w / 2.0 + fsz * 1.2 + text.chars().count() as f64 * fsz * 0.5 + 4.0 * k); cl.push(c); } tcur += d; on = !on; }
    let ids = push(ss, &tid, cl)?; Ok(json!({"clip_ids": ids}))
}
