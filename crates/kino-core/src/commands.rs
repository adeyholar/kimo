//! Command registry: single source of truth for every editor operation.
//! The MCP server lists and dispatches straight from `registry()`.
use crate::effects::CATALOG;
use crate::effects2::CATALOG2;
use crate::color::catalog3;
use crate::keyframe::{Ease, Prop};
use crate::model::*;
use crate::render;
use serde_json::{json, Value};

pub struct Session { pub project: Project, pub undo: Vec<Project>, pub redo: Vec<Project>, pub cache: render::Cache }
impl Session { pub fn new() -> Self { Session { project: Project::default(), undo: vec![], redo: vec![], cache: render::Cache::new() } } }

pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
    pub mutates: bool,
    pub run: fn(&mut Session, &Value) -> Result<Value, String>,
}

fn s(desc: &str) -> Value { json!({"type":"string","description":desc}) }
fn n(desc: &str) -> Value { json!({"type":"number","description":desc}) }
fn obj(props: Vec<(&str, Value)>, req: &[&str]) -> Value {
    let mut m = serde_json::Map::new(); for (k, v) in props { m.insert(k.into(), v); }
    json!({"type":"object","properties":m,"required":req})
}
fn str_arg<'a>(a: &'a Value, k: &str) -> Result<&'a str, String> { a.get(k).and_then(|v| v.as_str()).ok_or(format!("missing string '{k}'")) }
fn num_arg(a: &Value, k: &str) -> Result<f64, String> { a.get(k).and_then(|v| v.as_f64()).ok_or(format!("missing number '{k}'")) }
fn opt_num(a: &Value, k: &str, d: f64) -> f64 { a.get(k).and_then(|v| v.as_f64()).unwrap_or(d) }
fn color(a: &Value, k: &str, d: [f64; 3]) -> [f64; 3] {
    a.get(k).and_then(|v| v.as_array()).filter(|x| x.len() == 3).map(|x| [x[0].as_f64().unwrap_or(0.0), x[1].as_f64().unwrap_or(0.0), x[2].as_f64().unwrap_or(0.0)]).unwrap_or(d)
}
fn clip<'a>(p: &'a mut Project, a: &Value) -> Result<&'a mut Clip, String> {
    let id = str_arg(a, "clip_id")?; p.find_clip_mut(id).ok_or(format!("no clip '{id}'"))
}

pub fn registry() -> Vec<Command> {
    let eff_kinds: Vec<&str> = CATALOG.iter().chain(CATALOG2.iter()).chain(catalog3().iter()).map(|c| c.0).collect();
    let mut v: Vec<Command> = vec![
    Command { name: "project_set", description: "Set project canvas size and frame rate (and optional name).", mutates: true,
        schema: obj(vec![("width", n("pixels")), ("height", n("pixels")), ("fps", n("frames per second")), ("name", s("project name"))], &[]),
        run: |ss, a| { let p = &mut ss.project;
            if let Some(v) = a.get("width").and_then(|v| v.as_u64()) { p.width = v as u32; }
            if let Some(v) = a.get("height").and_then(|v| v.as_u64()) { p.height = v as u32; }
            if let Some(v) = a.get("fps").and_then(|v| v.as_f64()) { p.fps = v; }
            if let Some(v) = a.get("name").and_then(|v| v.as_str()) { p.name = v.into(); }
            Ok(json!({"width":p.width,"height":p.height,"fps":p.fps})) } },
    Command { name: "import_media", description: "Import a video file as an asset (probes size/duration). Returns asset id.", mutates: true,
        schema: obj(vec![("path", s("absolute path to media file"))], &["path"]),
        run: |ss, a| { let path = str_arg(a, "path")?; let (w, h, d, fps) = render::probe(path)?;
            let id = ss.project.gen_id("asset"); ss.project.assets.insert(id.clone(), Asset { id: id.clone(), path: path.into(), width: w, height: h, duration: d, fps });
            Ok(json!({"asset_id":id,"width":w,"height":h,"duration":d,"fps":fps})) } },
    Command { name: "add_track", description: "Add a track on top of the stack. Higher tracks composite over lower ones.", mutates: true,
        schema: obj(vec![], &[]),
        run: |ss, _| { let id = ss.project.gen_id("track"); ss.project.tracks.push(Track { id: id.clone(), clips: vec![], locked: false, muted: false, name: String::new() }); Ok(json!({"track_id":id})) } },
    Command { name: "add_clip", description: "Add a clip to a track. source_type: media (needs asset_id), solid (color [r,g,b] 0..1), gradient (color, color2).", mutates: true,
        schema: obj(vec![("track_id", s("target track")), ("source_type", json!({"type":"string","enum":["media","solid","gradient"]})), ("asset_id", s("for media")),
            ("color", json!({"type":"array","items":{"type":"number"},"description":"[r,g,b] 0..1"})), ("color2", json!({"type":"array","items":{"type":"number"}})),
            ("start", n("timeline start seconds")), ("duration", n("seconds")), ("in_point", n("source offset seconds")), ("speed", n("playback speed, default 1"))], &["track_id","source_type","start","duration"]),
        run: |ss, a| { let tid = str_arg(a, "track_id")?.to_string();
            let src = match str_arg(a, "source_type")? {
                "media" => { let id = str_arg(a, "asset_id")?; if !ss.project.assets.contains_key(id) { return Err(format!("no asset '{id}'")); } Source::Media { asset: id.into() } }
                "solid" => Source::Solid { color: color(a, "color", [0.0; 3]) },
                "gradient" => Source::Gradient { c1: color(a, "color", [0.0; 3]), c2: color(a, "color2", [1.0; 3]) },
                o => return Err(format!("bad source_type {o}")) };
            let id = ss.project.gen_id("clip");
            let c = Clip { id: id.clone(), source: src, start: num_arg(a, "start")?, duration: num_arg(a, "duration")?, in_point: opt_num(a, "in_point", 0.0), speed: opt_num(a, "speed", 1.0), speed_curve: None, props: default_props(), effects: vec![], blend: "normal".into(), mask: None, time_mode: TimeMode::Normal, corners: None, audio: Audio::default(), motion_blur: 0.0, text_fx: None, transition_in: None };
            let tr = ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or(format!("no track '{tid}'"))?; tr.clips.push(c);
            Ok(json!({"clip_id":id})) } },
    Command { name: "add_text", description: "Add a text clip (rendered with a TTF/OTF font). Animate with set_keyframe on reveal (typewriter), opacity, scale, x, y, rotation. Default font is a system sans.", mutates: true,
        schema: obj(vec![("track_id", s("track")), ("text", s("text, \\n for newlines")), ("start", n("seconds")), ("duration", n("seconds")), ("font", s("path to .ttf/.otf (optional)")), ("size", n("pixels, default 72")),
            ("color", json!({"type":"array","items":{"type":"number"}})), ("letter_spacing", n("pixels")), ("stroke_width", n("pixels")), ("stroke_color", json!({"type":"array","items":{"type":"number"}})), ("shadow", n("shadow offset pixels")), ("align", json!({"type":"string","enum":["left","center","right"]}))], &["track_id","text","start","duration"]),
        run: |ss, a| { let tid = str_arg(a, "track_id")?.to_string(); let id = ss.project.gen_id("clip");
            let src = Source::Text { text: str_arg(a, "text")?.replace("\\n", "\n"), font: a.get("font").and_then(|v| v.as_str()).unwrap_or("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf").into(), size: opt_num(a, "size", 72.0), color: color(a, "color", [1.0; 3]),
                letter_spacing: opt_num(a, "letter_spacing", 0.0), stroke_width: opt_num(a, "stroke_width", 0.0), stroke_color: color(a, "stroke_color", [0.0; 3]), shadow: opt_num(a, "shadow", 0.0), align: a.get("align").and_then(|v| v.as_str()).unwrap_or("center").into() };
            let c = Clip { id: id.clone(), source: src, start: num_arg(a, "start")?, duration: num_arg(a, "duration")?, in_point: 0.0, speed: 1.0, speed_curve: None, props: default_props(), effects: vec![], blend: "normal".into(), mask: None, time_mode: TimeMode::Normal, corners: None, audio: Audio::default(), motion_blur: 0.0, text_fx: None, transition_in: None };
            ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or("no such track")?.clips.push(c); Ok(json!({"clip_id":id})) } },
    Command { name: "set_text", description: "Edit text content/style of a text clip.", mutates: true,
        schema: obj(vec![("clip_id", s("text clip")), ("text", s("new text")), ("size", n("pixels")), ("color", json!({"type":"array","items":{"type":"number"}}))], &["clip_id"]),
        run: |ss, a| { let c = clip(&mut ss.project, a)?; if let Source::Text { text, size, color: col, .. } = &mut c.source {
            if let Some(t) = a.get("text").and_then(|v| v.as_str()) { *text = t.replace("\\n", "\n"); } if let Some(v) = a.get("size").and_then(|v| v.as_f64()) { *size = v; }
            if a.get("color").is_some() { *col = color(a, "color", *col); } Ok(json!({"ok":true})) } else { Err("not a text clip".into()) } } },
    Command { name: "set_mask", description: "Set a mask on a clip: shape rect|ellipse|linear, normalized cx,cy,w,h (0..1), feather (0..1), rotation (deg), invert. Mask params are keyframable via set_mask_keyframe.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("shape", json!({"type":"string","enum":["rect","ellipse","linear"]})), ("cx", n("0..1")), ("cy", n("0..1")), ("w", n("0..1")), ("h", n("0..1")), ("feather", n("0..1")), ("rotation", n("degrees")), ("invert", json!({"type":"boolean"}))], &["clip_id","shape"]),
        run: |ss, a| { let shape = str_arg(a, "shape")?.to_string(); let mut props = std::collections::BTreeMap::new();
            for (k, d) in [("cx",0.5),("cy",0.5),("w",0.5),("h",0.5),("feather",0.02),("rotation",0.0)] { props.insert(k.to_string(), Prop::new(opt_num(a, k, d))); }
            let inv = a.get("invert").and_then(|v| v.as_bool()).unwrap_or(false); clip(&mut ss.project, a)?.mask = Some(Mask { shape, props, invert: inv }); Ok(json!({"ok":true})) } },
    Command { name: "set_mask_keyframe", description: "Keyframe a mask param (cx, cy, w, h, feather, rotation) at clip-local time.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("property", s("mask param")), ("time", n("seconds")), ("value", n("value")), ("ease", json!({"type":"string","enum":["linear","in","out","in_out","hold"]}))], &["clip_id","property","time","value"]),
        run: |ss, a| { let k = str_arg(a, "property")?.to_string(); let t = num_arg(a, "time")?; let v = num_arg(a, "value")?; let e: Ease = serde_json::from_value(a.get("ease").cloned().unwrap_or(json!("linear"))).map_err(|e| e.to_string())?;
            let m = clip(&mut ss.project, a)?.mask.as_mut().ok_or("clip has no mask")?; m.props.entry(k).or_insert(Prop::new(v)).set_key(t, v, e); Ok(json!({"ok":true})) } },
    Command { name: "remove_mask", description: "Remove a clip's mask.", mutates: true,
        schema: obj(vec![("clip_id", s("clip"))], &["clip_id"]),
        run: |ss, a| { clip(&mut ss.project, a)?.mask = None; Ok(json!({"ok":true})) } },
    Command { name: "set_audio", description: "Audio settings for a media clip: mute, volume (base), fade_in/fade_out seconds, pitch (semitones), highpass/lowpass Hz, denoise (dB, 0=off), compress, echo (seconds delay), reverse. Volume is keyframable via set_audio_volume_key.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("mute", json!({"type":"boolean"})), ("volume", n("0..4")), ("fade_in", n("s")), ("fade_out", n("s")), ("pitch", n("semitones")), ("highpass", n("Hz")), ("lowpass", n("Hz")), ("denoise", n("dB")), ("compress", json!({"type":"boolean"})), ("echo", n("s")), ("reverse", json!({"type":"boolean"}))], &["clip_id"]),
        run: |ss, a| { let c = clip(&mut ss.project, a)?; let au = &mut c.audio;
            if let Some(v) = a.get("mute").and_then(|v| v.as_bool()) { au.mute = v; } if let Some(v) = a.get("volume").and_then(|v| v.as_f64()) { au.volume.base = v; }
            for (k, r) in [("fade_in", &mut au.fade_in), ("fade_out", &mut au.fade_out), ("pitch", &mut au.pitch), ("highpass", &mut au.highpass), ("lowpass", &mut au.lowpass), ("denoise", &mut au.denoise), ("echo", &mut au.echo)] { if let Some(v) = a.get(k).and_then(|v| v.as_f64()) { *r = v; } }
            if let Some(v) = a.get("compress").and_then(|v| v.as_bool()) { au.compress = v; } if let Some(v) = a.get("reverse").and_then(|v| v.as_bool()) { au.reverse = v; }
            Ok(serde_json::to_value(&*au).unwrap()) } },
    Command { name: "set_audio_volume_key", description: "Keyframe clip volume at clip-local time (ducking, swells).", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("time", n("s")), ("value", n("gain")), ("ease", json!({"type":"string","enum":["linear","in","out","in_out","hold"]}))], &["clip_id","time","value"]),
        run: |ss, a| { let e: Ease = serde_json::from_value(a.get("ease").cloned().unwrap_or(json!("linear"))).map_err(|e| e.to_string())?; let (t, v) = (num_arg(a, "time")?, num_arg(a, "value")?); clip(&mut ss.project, a)?.audio.volume.set_key(t, v, e); Ok(json!({"ok":true})) } },
    Command { name: "detect_beats", description: "Detect beat/onset times (seconds) in an audio or video file, for cutting on the beat. Returns times plus tempo estimate.", mutates: false,
        schema: obj(vec![("path", s("media path")), ("min_gap", n("min seconds between beats, default 0.25"))], &["path"]),
        run: |_, a| { let b = crate::audio::detect_beats(str_arg(a, "path")?, opt_num(a, "min_gap", 0.25)); let bpm = if b.len() > 3 { let mut d: Vec<f64> = b.windows(2).map(|w| w[1] - w[0]).collect(); d.sort_by(|x, y| x.partial_cmp(y).unwrap()); 60.0 / d[d.len() / 2] } else { 0.0 }; Ok(json!({"beats":b,"bpm":bpm})) } },
    Command { name: "export_audio", description: "Mix all clip audio to a 16-bit 48k stereo WAV.", mutates: false,
        schema: obj(vec![("path", s("output .wav"))], &["path"]),
        run: |ss, a| { let p = str_arg(a, "path")?; crate::audio::write_wav(p, &crate::audio::mix(&ss.project))?; Ok(json!({"path":p})) } },
    Command { name: "set_speed_ramp", description: "Speed ramp: keyframed playback speed over clip-local time (e.g. [[0,1],[1,0.2],[2,3]]). Replaces constant speed for video. Pass empty keys to clear.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("keys", json!({"type":"array","items":{"type":"array","items":{"type":"number"},"description":"[time, speed]"}})), ("ease", json!({"type":"string","enum":["linear","in","out","in_out","hold"]}))], &["clip_id","keys"]),
        run: |ss, a| { let e: Ease = serde_json::from_value(a.get("ease").cloned().unwrap_or(json!("in_out"))).map_err(|e| e.to_string())?;
            let ks: Vec<(f64, f64)> = a.get("keys").and_then(|v| v.as_array()).ok_or("keys required")?.iter().filter_map(|k| { let k = k.as_array()?; Some((k.get(0)?.as_f64()?, k.get(1)?.as_f64()?)) }).collect();
            let c = clip(&mut ss.project, a)?; if ks.is_empty() { c.speed_curve = None; return Ok(json!({"cleared":true})); }
            let mut p = Prop::new(ks[0].1); for (t, v) in &ks { if !(0.0..=100.0).contains(v) { return Err("speed must be 0..100".into()); } p.set_key(*t, *v, e); } c.speed_curve = Some(p);
            Ok(json!({"source_seconds": c.source_time(c.duration)})) } },
    Command { name: "track_point", description: "Motion-track a surface point on a media clip and attach a target clip (text, graphic) so it sticks. rigid=true (default) tracks a ring of patches and also solves rotation and scale, so attached graphics rotate/scale with the surface; rigid=false tracks translation only. Writes x/y (+rotation, scale) keyframes on target_clip_id. Memory is windowed.", mutates: true,
        schema: obj(vec![("clip_id", s("media clip to track")), ("target_clip_id", s("clip to attach")), ("x", n("0..1 point x at start")), ("y", n("0..1 point y at start")), ("patch", n("patch size px, default 40")), ("search", n("search radius px per frame, default 14")), ("rigid", json!({"type":"boolean"})), ("radius", n("ring radius as fraction of min dimension, default 0.12")), ("step", n("keyframe every N frames, default 1"))], &["clip_id","target_clip_id","x","y"]),
        run: |ss, a| { let tid = str_arg(a, "target_clip_id")?.to_string(); let (nx, ny) = (num_arg(a, "x")?, num_arg(a, "y")?); let rigid = a.get("rigid").and_then(|v| v.as_bool()).unwrap_or(true);
            let tr = crate::tracking::track_point(&ss.project, str_arg(a, "clip_id")?, nx, ny, opt_num(a, "patch", 40.0) as i32, opt_num(a, "search", 14.0) as i32, rigid, opt_num(a, "radius", 0.12))?;
            let (w, h) = (ss.project.width as f64, ss.project.height as f64); let step = opt_num(a, "step", 1.0).max(1.0) as usize;
            let src = ss.project.find_clip(str_arg(a, "clip_id")?).unwrap().start; let t0 = ss.project.find_clip(&tid).ok_or("no target clip")?.start;
            let tc = ss.project.find_clip_mut(&tid).ok_or("no target clip")?; let (bx, by) = (tc.props["x"].base, tc.props["y"].base); let (br, bs) = (tc.props.get("rotation").map(|p| p.base).unwrap_or(0.0), tc.props.get("scale").map(|p| p.base).unwrap_or(1.0));
            let (mut kx, mut ky, mut kr, mut ks) = (Prop::new(bx), Prop::new(by), Prop::new(br), Prop::new(bs)); let n = tr.len();
            for (i, s_) in tr.iter().enumerate() { if i % step != 0 && i != n - 1 { continue; } let lt = s_.t + src - t0; kx.set_key(lt, bx + s_.x - nx * w, Ease::Linear); ky.set_key(lt, by + s_.y - ny * h, Ease::Linear); if rigid { kr.set_key(lt, br + s_.rot_deg, Ease::Linear); ks.set_key(lt, bs * s_.scale, Ease::Linear); } }
            tc.props.insert("x".into(), kx); tc.props.insert("y".into(), ky); if rigid { tc.props.insert("rotation".into(), kr); tc.props.insert("scale".into(), ks); } let e = &tr[n-1]; Ok(json!({"frames":n,"end_px":[e.x, e.y],"end_rotation_deg":e.rot_deg,"end_scale":e.scale})) } },
    Command { name: "stabilize", description: "Two-pass stabilizer: robust similarity (translation + rotation + scale) camera-path estimate from a patch grid, Gaussian path smoothing, adaptive zoom to hide borders. smooth = sigma in frames (bigger = steadier, ~10 default); lock=true holds the camera fixed (tripod); rotation=false corrects translation only. Writes x/y/rotation/scale keyframes.", mutates: true,
        schema: obj(vec![("clip_id", s("media clip")), ("smooth", n("sigma in frames, default 10")), ("lock", json!({"type":"boolean"})), ("rotation", json!({"type":"boolean"}))], &["clip_id"]),
        run: |ss, a| { let id = str_arg(a, "clip_id")?.to_string(); let off = crate::tracking::stabilize(&ss.project, &id, opt_num(a, "smooth", 10.0), a.get("lock").and_then(|v| v.as_bool()).unwrap_or(false), a.get("rotation").and_then(|v| v.as_bool()).unwrap_or(true))?;
            let c = ss.project.find_clip_mut(&id).unwrap(); let (mut kx, mut ky, mut kr, mut ks) = (Prop::new(0.0), Prop::new(0.0), Prop::new(0.0), Prop::new(1.0)); let (mut mx, mut mr, mut mz) = (0f64, 0f64, 1f64);
            for s_ in &off { kx.set_key(s_.t, s_.dx, Ease::Linear); ky.set_key(s_.t, s_.dy, Ease::Linear); kr.set_key(s_.t, s_.rot_deg, Ease::Linear); ks.set_key(s_.t, s_.scale, Ease::Linear); mx = mx.max(s_.dx.abs()).max(s_.dy.abs()); mr = mr.max(s_.rot_deg.abs()); mz = mz.max(s_.scale); }
            c.props.insert("x".into(), kx); c.props.insert("y".into(), ky); c.props.insert("rotation".into(), kr); c.props.insert("scale".into(), ks); Ok(json!({"frames":off.len(),"max_translation_px":mx,"max_rotation_deg":mr,"max_zoom":mz})) } },
    Command { name: "list_transitions", description: "List available transition kinds.", mutates: false, schema: obj(vec![], &[]),
        run: |_, _| Ok(json!(["crossfade","wipe_left","wipe_right","wipe_up","wipe_down","diagonal","blinds","checker","iris","slide_left","slide_right","slide_up","slide_down","zoom","spin","flash"])) },
    Command { name: "import_srt", description: "Create caption text clips from an SRT file on a track. Style args apply to every caption. Each cue becomes a text clip at its time range.", mutates: true,
        schema: obj(vec![("track_id", s("track")), ("path", s("SRT file path")), ("size", n("px, default 48")), ("color", json!({"type":"array","items":{"type":"number"}})), ("font", s("ttf path")), ("stroke_width", n("px")), ("y", n("vertical offset px, default 220 (lower third)"))], &["track_id","path"]),
        run: |ss, a| { let txt = std::fs::read_to_string(str_arg(a, "path")?).map_err(|e| e.to_string())?; let tid = str_arg(a, "track_id")?.to_string(); let mut made = vec![];
            for blk in txt.replace("\r", "").split("\n\n") { let ls: Vec<&str> = blk.lines().filter(|l| !l.trim().is_empty()).collect(); let ti = ls.iter().position(|l| l.contains("-->")); let Some(ti) = ti else { continue };
                let pt = |s: &str| -> f64 { let s = s.trim().replace(',', "."); let p: Vec<f64> = s.split(':').filter_map(|x| x.parse().ok()).collect(); if p.len() == 3 { p[0]*3600.0 + p[1]*60.0 + p[2] } else { 0.0 } };
                let mut it = ls[ti].split("-->"); let (st, en) = (pt(it.next().unwrap_or("")), pt(it.next().unwrap_or(""))); if en <= st { continue; }
                let body = ls[ti + 1..].join("\\n"); let id = ss.project.gen_id("clip");
                let src = Source::Text { text: body.replace("\\n", "\n"), font: a.get("font").and_then(|v| v.as_str()).unwrap_or("/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf").into(), size: opt_num(a, "size", 48.0), color: color(a, "color", [1.0; 3]), letter_spacing: 0.0, stroke_width: opt_num(a, "stroke_width", 2.0), stroke_color: [0.0; 3], shadow: 2.0, align: "center".into() };
                let mut props = default_props(); props.get_mut("y").unwrap().base = opt_num(a, "y", 220.0);
                let c = Clip { id: id.clone(), source: src, start: st, duration: en - st, in_point: 0.0, speed: 1.0, speed_curve: None, props, effects: vec![], blend: "normal".into(), mask: None, time_mode: TimeMode::Normal, corners: None, audio: Audio::default(), motion_blur: 0.0, text_fx: None, transition_in: None };
                ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or("no such track")?.clips.push(c); made.push(id); }
            Ok(json!({"captions":made.len(),"clip_ids":made})) } },
    Command { name: "get_scopes", description: "Video scopes at a time: per-channel histogram (32 bins), luma stats, clipping %, average color, and a coarse luma waveform - for AI color grading feedback.", mutates: false,
        schema: obj(vec![("time", n("seconds"))], &["time"]),
        run: |ss, a| { let tt = num_arg(a, "time")?; render::prepare_at(&ss.project, &mut ss.cache, Some(tt))?; let f = render::render_frame(&ss.project, tt, &ss.cache)?;
            let n = f.w * f.h; let mut hist = [[0u32; 32]; 3]; let (mut sum, mut lo, mut hi) = ([0f64; 3], 0u32, 0u32); let mut lsum = 0f64; let mut lsq = 0f64;
            for i in 0..n { let mut l = 0f32; for j in 0..3 { let v = f.d[i*4+j].clamp(0.0, 1.0); hist[j][((v * 31.999) as usize).min(31)] += 1; sum[j] += v as f64; l += v * [0.2126, 0.7152, 0.0722][j]; } lsum += l as f64; lsq += (l * l) as f64; if l < 0.02 { lo += 1; } if l > 0.98 { hi += 1; } }
            let mean = lsum / n as f64; let cols = 16; let mut wf = vec![]; for cx in 0..cols { let (mut mn, mut mx) = (1f32, 0f32); for y in (0..f.h).step_by(4) { for x in (cx * f.w / cols..(cx + 1) * f.w / cols).step_by(4) { let i = y * f.w + x; let l = 0.2126*f.d[i*4] + 0.7152*f.d[i*4+1] + 0.0722*f.d[i*4+2]; mn = mn.min(l); mx = mx.max(l); } } wf.push(json!([mn, mx])); }
            Ok(json!({"avg_rgb":[sum[0]/n as f64, sum[1]/n as f64, sum[2]/n as f64],"luma_mean":mean,"luma_std":(lsq/n as f64 - mean*mean).max(0.0).sqrt(),"crushed_blacks_pct":lo as f64*100.0/n as f64,"clipped_whites_pct":hi as f64*100.0/n as f64,"histogram_r":hist[0],"histogram_g":hist[1],"histogram_b":hist[2],"luma_waveform_min_max_per_column":wf})) } },
    Command { name: "add_particles", description: "Add a procedural particle layer clip: kind sparks|embers|snow|rain|dust. Transparent background; blend with add/screen for glow. Combine with glow effect.", mutates: true,
        schema: obj(vec![("track_id", s("track")), ("kind", json!({"type":"string","enum":["sparks","embers","snow","rain","dust"]})), ("start", n("s")), ("duration", n("s")), ("count", n("particles, default 200")), ("color", json!({"type":"array","items":{"type":"number"}})), ("size", n("px radius, default 3")), ("seed", n("random seed"))], &["track_id","kind","start","duration"]),
        run: |ss, a| { let tid = str_arg(a, "track_id")?.to_string(); let id = ss.project.gen_id("clip");
            let src = Source::Particles { kind: str_arg(a, "kind")?.into(), count: opt_num(a, "count", 200.0) as u32, color: color(a, "color", [1.0, 0.7, 0.3]), size: opt_num(a, "size", 3.0), seed: opt_num(a, "seed", 1.0) as u64 };
            let mut c = Clip { id: id.clone(), source: src, start: num_arg(a, "start")?, duration: num_arg(a, "duration")?, in_point: 0.0, speed: 1.0, speed_curve: None, props: default_props(), effects: vec![], blend: "add".into(), mask: None, time_mode: TimeMode::Normal, corners: None, audio: Audio::default(), motion_blur: 0.0, text_fx: None, transition_in: None }; c.blend = "add".into();
            ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or("no such track")?.clips.push(c); Ok(json!({"clip_id":id})) } },
    Command { name: "transcribe", description: "Speech-to-text via the `whisper` CLI (openai-whisper or whisper.cpp wrapper) if installed; writes an SRT you can pass to import_srt. Fails with a clear message when whisper is not installed.", mutates: false,
        schema: obj(vec![("path", s("audio/video file")), ("out_srt", s("output .srt path")), ("model", s("model name, default base"))], &["path","out_srt"]),
        run: |_, a| { let (inp, out) = (str_arg(a, "path")?, str_arg(a, "out_srt")?); let dir = std::path::Path::new(out).parent().map(|p| p.to_string_lossy().to_string()).filter(|s| !s.is_empty()).unwrap_or(".".into());
            let st = std::process::Command::new("whisper").args([inp, "--model", a.get("model").and_then(|v| v.as_str()).unwrap_or("base"), "--output_format", "srt", "--output_dir", &dir]).status().map_err(|_| "whisper CLI not found: install openai-whisper (pip install openai-whisper) and retry".to_string())?;
            if !st.success() { return Err("whisper failed".into()); } let stem = std::path::Path::new(inp).file_stem().unwrap().to_string_lossy().to_string(); let produced = format!("{dir}/{stem}.srt"); if produced != out { std::fs::rename(&produced, out).map_err(|e| e.to_string())?; } Ok(json!({"srt":out})) } },
    Command { name: "export_gif", description: "Export a time range as an animated GIF.", mutates: false,
        schema: obj(vec![("path", s("output .gif")), ("start", n("s")), ("end", n("s")), ("fps", n("default 15"))], &["path"]),
        run: |ss, a| { let tmp = format!("{}.tmp.mp4", str_arg(a, "path")?); render::export(&ss.project, &tmp, 20)?; let (st, en) = (opt_num(a, "start", 0.0), opt_num(a, "end", ss.project.duration()));
            let ok = std::process::Command::new("ffmpeg").args(["-v","error","-y","-ss",&st.to_string(),"-t",&(en-st).to_string(),"-i",&tmp,"-vf",&format!("fps={},split[a][b];[a]palettegen[p];[b][p]paletteuse", opt_num(a, "fps", 15.0)),str_arg(a, "path")?]).status().map_err(|e| e.to_string())?.success();
            let _ = std::fs::remove_file(&tmp); if ok { Ok(json!({"path":str_arg(a, "path")?})) } else { Err("gif encode failed".into()) } } },
    Command { name: "add_adjustment_layer", description: "Add an adjustment layer: effects (grade, glow, glitch...) added to it apply to everything beneath it for its duration. Use add_effect on the returned clip_id.", mutates: true,
        schema: obj(vec![("track_id", s("track (put on top)")), ("start", n("s")), ("duration", n("s"))], &["track_id","start","duration"]),
        run: |ss, a| { let tid = str_arg(a, "track_id")?.to_string(); let id = ss.project.gen_id("clip");
            let c = Clip { id: id.clone(), source: Source::Adjustment, start: num_arg(a, "start")?, duration: num_arg(a, "duration")?, in_point: 0.0, speed: 1.0, speed_curve: None, props: default_props(), effects: vec![], blend: "normal".into(), mask: None, time_mode: TimeMode::Normal, corners: None, audio: Audio::default(), motion_blur: 0.0, text_fx: None, transition_in: None };
            ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or("no such track")?.clips.push(c); Ok(json!({"clip_id":id})) } },
    Command { name: "add_shape", description: "Add a vector shape layer: rect, ellipse, polygon (sides), line. Pixel size, fill + stroke, corner_radius for rects. Move with set_property x/y; animate with keyframes.", mutates: true,
        schema: obj(vec![("track_id", s("track")), ("shape", json!({"type":"string","enum":["rect","ellipse","polygon","line"]})), ("start", n("s")), ("duration", n("s")), ("width", n("px")), ("height", n("px")), ("fill", json!({"type":"array","items":{"type":"number"}})), ("stroke_width", n("px")), ("stroke_color", json!({"type":"array","items":{"type":"number"}})), ("sides", n("polygon sides")), ("corner_radius", n("px"))], &["track_id","shape","start","duration"]),
        run: |ss, a| { let tid = str_arg(a, "track_id")?.to_string(); let id = ss.project.gen_id("clip");
            let src = Source::Shape { shape: str_arg(a, "shape")?.into(), width: opt_num(a, "width", 300.0), height: opt_num(a, "height", 200.0), fill: color(a, "fill", [1.0; 3]), stroke_width: opt_num(a, "stroke_width", 0.0), stroke_color: color(a, "stroke_color", [0.0; 3]), sides: opt_num(a, "sides", 5.0) as u32, corner_radius: opt_num(a, "corner_radius", 0.0) };
            let c = Clip { id: id.clone(), source: src, start: num_arg(a, "start")?, duration: num_arg(a, "duration")?, in_point: 0.0, speed: 1.0, speed_curve: None, props: default_props(), effects: vec![], blend: "normal".into(), mask: None, time_mode: TimeMode::Normal, corners: None, audio: Audio::default(), motion_blur: 0.0, text_fx: None, transition_in: None };
            ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or("no such track")?.clips.push(c); Ok(json!({"clip_id":id})) } },
    Command { name: "set_motion_blur", description: "Per-clip motion blur from keyframed/tracked transform motion. shutter_angle in degrees (180 = film standard, 0 = off). Multi-sample accumulation of x/y/scale/rotation across the shutter interval.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("shutter_angle", n("0..720, default 180"))], &["clip_id"]),
        run: |ss, a| { let v = opt_num(a, "shutter_angle", 180.0).clamp(0.0, 720.0); clip(&mut ss.project, a)?.motion_blur = v; Ok(json!({"shutter_angle": v})) } },
    Command { name: "set_text_style", description: "Rich text styling for a text clip: box_width (word wrap, 0=off), line_spacing, kerning, background box (bg [r,g,b,a], bg_pad, bg_radius), soft shadow (shadow_blur, shadow_opacity, shadow_color, shadow_angle deg), vertical gradient fill (gradient_top/gradient_bottom rgb, or empty to clear), glow (px radius) + glow_color. Only provided fields change.", mutates: true,
        schema: obj(vec![("clip_id", s("text clip")), ("box_width", n("px")), ("line_spacing", n("multiplier, default 1.15")), ("kerning", json!({"type":"boolean"})), ("bg", json!({"type":"array","items":{"type":"number"},"description":"[r,g,b,a] or [] to clear"})), ("bg_pad", n("px")), ("bg_radius", n("px")), ("shadow_blur", n("px")), ("shadow_opacity", n("0..1")), ("shadow_color", json!({"type":"array","items":{"type":"number"}})), ("shadow_angle", n("degrees, 45 = down-right")), ("gradient_top", json!({"type":"array","items":{"type":"number"}})), ("gradient_bottom", json!({"type":"array","items":{"type":"number"}})), ("glow", n("px radius, 0 off")), ("glow_color", json!({"type":"array","items":{"type":"number"}}))], &["clip_id"]),
        run: |ss, a| { let c = clip(&mut ss.project, a)?; let fx = c.text_fx.get_or_insert_with(TextFx::default);
            let rgb = |k: &str| a.get(k).and_then(|v| v.as_array()).filter(|v| v.len() >= 3).map(|v| [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0), v[2].as_f64().unwrap_or(0.0)]);
            if a.get("box_width").is_some() { fx.box_width = opt_num(a, "box_width", 0.0); } if a.get("line_spacing").is_some() { fx.line_spacing = opt_num(a, "line_spacing", 1.15); }
            if let Some(k) = a.get("kerning").and_then(|v| v.as_bool()) { fx.kerning = k; }
            if let Some(b) = a.get("bg").and_then(|v| v.as_array()) { fx.bg = if b.len() >= 3 { Some([b[0].as_f64().unwrap_or(0.0), b[1].as_f64().unwrap_or(0.0), b[2].as_f64().unwrap_or(0.0), b.get(3).and_then(|x| x.as_f64()).unwrap_or(1.0)]) } else { None }; }
            if a.get("bg_pad").is_some() { fx.bg_pad = opt_num(a, "bg_pad", 24.0); } if a.get("bg_radius").is_some() { fx.bg_radius = opt_num(a, "bg_radius", 12.0); }
            if a.get("shadow_blur").is_some() { fx.shadow_blur = opt_num(a, "shadow_blur", 0.0); } if a.get("shadow_opacity").is_some() { fx.shadow_opacity = opt_num(a, "shadow_opacity", 0.6); } if a.get("shadow_angle").is_some() { fx.shadow_angle = opt_num(a, "shadow_angle", 45.0); }
            if let Some(v) = rgb("shadow_color") { fx.shadow_color = v; } if let Some(v) = rgb("glow_color") { fx.glow_color = v; } if a.get("glow").is_some() { fx.glow = opt_num(a, "glow", 0.0); }
            match (rgb("gradient_top"), rgb("gradient_bottom")) { (Some(t), Some(b)) => fx.gradient = Some((t, b)), _ => { if a.get("gradient_top").map(|v| v.as_array().map(|x| x.is_empty()).unwrap_or(false)).unwrap_or(false) { fx.gradient = None; } } }
            Ok(json!({"ok":true})) } },
    Command { name: "add_text_animator", description: "After Effects-style text animator. unit char|word|line; a range selector (start,end 0..1, shape square|ramp_up|ramp_down|triangle|round|smooth, smoothness) selects units; units inside get x/y offset (px), scale (1=none), rotation (deg), opacity (1=none), tracking (px), blur. Key the selector with set_animator_keyframe prop=offset. Or use `preset` for ready-made: reveal, rise, drop, pop, wave, cascade_rotate, blur_in, typewriter, tracking_in (with `duration` seconds, `delay`, `smoothness`).", mutates: true,
        schema: obj(vec![("clip_id", s("text clip")), ("preset", json!({"type":"string","enum":["reveal","rise","drop","pop","wave","cascade_rotate","blur_in","typewriter","tracking_in"]})), ("unit", json!({"type":"string","enum":["char","word","line"]})), ("shape", s("selector shape")), ("start", n("0..1")), ("end", n("0..1")), ("smoothness", n("0..1")), ("x", n("px")), ("y", n("px")), ("scale", n("")), ("rotation", n("deg")), ("opacity", n("")), ("tracking", n("px")), ("blur", n("px")), ("duration", n("preset sweep seconds")), ("delay", n("preset start delay, clip-local s")), ("out", json!({"type":"boolean","description":"preset plays out instead of in"}))], &["clip_id"]),
        run: |ss, a| { let c = clip(&mut ss.project, a)?; let fx = c.text_fx.get_or_insert_with(TextFx::default);
            let dur = opt_num(a, "duration", 0.8).max(0.05); let delay = opt_num(a, "delay", 0.0); let sm = opt_num(a, "smoothness", 0.3); let out = a.get("out").and_then(|v| v.as_bool()).unwrap_or(false);
            let id = format!("an{}", fx.animators.len() + 1);
            let mut an = TextAnimator { id: id.clone(), unit: a.get("unit").and_then(|v| v.as_str()).unwrap_or("char").into(), shape: a.get("shape").and_then(|v| v.as_str()).unwrap_or("square").into(), start: Prop::new(opt_num(a, "start", 0.0)), end: Prop::new(opt_num(a, "end", 1.0)), offset: Prop::new(0.0), smoothness: Prop::new(sm), x: Prop::new(opt_num(a, "x", 0.0)), y: Prop::new(opt_num(a, "y", 0.0)), scale: Prop::new(opt_num(a, "scale", 1.0)), rotation: Prop::new(opt_num(a, "rotation", 0.0)), opacity: Prop::new(opt_num(a, "opacity", 1.0)), tracking: Prop::new(opt_num(a, "tracking", 0.0)), blur: Prop::new(opt_num(a, "blur", 0.0)) };
            if let Some(p) = a.get("preset").and_then(|v| v.as_str()) {
                match p { "reveal" | "typewriter" => { an.opacity = Prop::new(0.0); if p == "typewriter" { an.smoothness = Prop::new(0.0); } }
                    "rise" => { an.y = Prop::new(70.0); an.opacity = Prop::new(0.0); an.unit = "char".into(); } "drop" => { an.y = Prop::new(-90.0); an.opacity = Prop::new(0.0); }
                    "pop" => { an.scale = Prop::new(0.0); an.opacity = Prop::new(0.0); an.unit = "word".into(); } "blur_in" => { an.blur = Prop::new(14.0); an.opacity = Prop::new(0.0); }
                    "cascade_rotate" => { an.rotation = Prop::new(-35.0); an.y = Prop::new(50.0); an.opacity = Prop::new(0.0); } "tracking_in" => { an.tracking = Prop::new(40.0); an.opacity = Prop::new(0.0); an.unit = "word".into(); }
                    "wave" => { an.shape = "triangle".into(); an.y = Prop::new(-40.0); an.start = Prop::new(0.0); an.end = Prop::new(0.35); an.smoothness = Prop::new(1.0); }
                    o => return Err(format!("unknown preset {o}")) }
                let hi = 1.0 + 2.0 * sm.max(0.05);
                if p == "wave" { an.offset.set_key(delay, -0.35, Ease::Linear); an.offset.set_key(delay + dur, 1.35, Ease::Linear); }
                else if out { an.offset.set_key(delay, -hi, Ease::Out); an.offset.set_key(delay + dur, 0.0, Ease::Out); }
                else { an.offset.set_key(delay, 0.0, Ease::InOut); an.offset.set_key(delay + dur, hi, Ease::InOut); }
                if p == "typewriter" { an.offset = Prop::new(0.0); let n = 1.0; let _ = n; an.offset.set_key(delay, 0.0, Ease::Linear); an.offset.set_key(delay + dur, 1.0, Ease::Linear); }
            }
            fx.animators.push(an); Ok(json!({"animator_id": id})) } },
    Command { name: "set_animator_keyframe", description: "Keyframe a text-animator property (clip-local seconds). prop: start|end|offset|smoothness|x|y|scale|rotation|opacity|tracking|blur.", mutates: true,
        schema: obj(vec![("clip_id", s("text clip")), ("animator_id", s("id from add_text_animator")), ("prop", s("property")), ("time", n("clip-local seconds")), ("value", n("")), ("ease", json!({"type":"string","enum":["linear","in","out","in_out","hold"]}))], &["clip_id","animator_id","prop","time","value"]),
        run: |ss, a| { let ease: Ease = serde_json::from_value(a.get("ease").cloned().unwrap_or(json!("in_out"))).map_err(|e| e.to_string())?; let (t, v) = (num_arg(a, "time")?, num_arg(a, "value")?); let aid = str_arg(a, "animator_id")?.to_string(); let pn = str_arg(a, "prop")?.to_string();
            let c = clip(&mut ss.project, a)?; let an = c.text_fx.as_mut().and_then(|f| f.animators.iter_mut().find(|x| x.id == aid)).ok_or("no such animator")?;
            let p = match pn.as_str() { "start" => &mut an.start, "end" => &mut an.end, "offset" => &mut an.offset, "smoothness" => &mut an.smoothness, "x" => &mut an.x, "y" => &mut an.y, "scale" => &mut an.scale, "rotation" => &mut an.rotation, "opacity" => &mut an.opacity, "tracking" => &mut an.tracking, "blur" => &mut an.blur, o => return Err(format!("bad prop {o}")) };
            p.set_key(t, v, ease); Ok(json!({"ok":true})) } },
    Command { name: "set_time_mode", description: "Time remap: mode normal | reverse | freeze (hold the frame at clip-local time `at`).", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("mode", json!({"type":"string","enum":["normal","reverse","freeze"]})), ("at", n("freeze time, clip-local seconds"))], &["clip_id","mode"]),
        run: |ss, a| { let m = match str_arg(a, "mode")? { "normal" => TimeMode::Normal, "reverse" => TimeMode::Reverse, "freeze" => TimeMode::Freeze { at: opt_num(a, "at", 0.0) }, o => return Err(format!("bad mode {o}")) }; clip(&mut ss.project, a)?.time_mode = m; Ok(json!({"ok":true})) } },
    Command { name: "freeze_frame", description: "Split-free freeze: hold the frame of a media clip at a timeline time for the rest of the clip.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("at", n("timeline seconds"))], &["clip_id","at"]),
        run: |ss, a| { let at = num_arg(a, "at")?; let c = clip(&mut ss.project, a)?; c.time_mode = TimeMode::Freeze { at: (at - c.start).max(0.0) }; Ok(json!({"ok":true})) } },
    Command { name: "set_corner_pin", description: "Perspective corner pin: pixel offsets of the 4 corners [[dx,dy] x4: top-left, top-right, bottom-right, bottom-left] from their normal positions. Pass null/empty to clear.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("corners", json!({"type":"array","items":{"type":"array","items":{"type":"number"}}}))], &["clip_id"]),
        run: |ss, a| { let cs = a.get("corners").and_then(|v| v.as_array()); let c = clip(&mut ss.project, a)?; match cs { Some(v) if v.len() == 4 => { let mut o = [[0.0; 2]; 4]; for (i, p) in v.iter().enumerate() { let p = p.as_array().ok_or("corner must be [dx,dy]")?; o[i] = [p.get(0).and_then(|x| x.as_f64()).unwrap_or(0.0), p.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0)]; } c.corners = Some(o); } _ => c.corners = None } Ok(json!({"ok":true})) } },
    Command { name: "move_clip", description: "Move a clip to a new timeline start time.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("start", n("seconds"))], &["clip_id","start"]),
        run: |ss, a| { let st = num_arg(a, "start")?; clip(&mut ss.project, a)?.start = st; Ok(json!({"ok":true})) } },
    Command { name: "trim_clip", description: "Set a clip's source in-point and/or duration.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("in_point", n("seconds")), ("duration", n("seconds"))], &["clip_id"]),
        run: |ss, a| { let c = clip(&mut ss.project, a)?; if let Some(v) = a.get("in_point").and_then(|v| v.as_f64()) { c.in_point = v; } if let Some(v) = a.get("duration").and_then(|v| v.as_f64()) { c.duration = v; } Ok(json!({"in_point":c.in_point,"duration":c.duration})) } },
    Command { name: "split_clip", description: "Split a clip at a timeline time into two clips. Returns the new right-hand clip id.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("at", n("timeline seconds"))], &["clip_id","at"]),
        run: |ss, a| { let at = num_arg(a, "at")?; let id = str_arg(a, "clip_id")?.to_string(); let nid = ss.project.gen_id("clip");
            let tr = ss.project.tracks.iter_mut().find(|t| t.clips.iter().any(|c| c.id == id)).ok_or("no such clip")?;
            let c = tr.clips.iter_mut().find(|c| c.id == id).unwrap();
            if at <= c.start || at >= c.start + c.duration { return Err("split point outside clip".into()); }
            let mut r = c.clone(); r.id = nid.clone(); r.start = at; r.duration = c.start + c.duration - at; r.in_point = c.in_point + (at - c.start) * c.speed; r.transition_in = None;
            c.duration = at - c.start; tr.clips.push(r); Ok(json!({"right_clip_id":nid})) } },
    Command { name: "delete_clip", description: "Delete a clip.", mutates: true,
        schema: obj(vec![("clip_id", s("clip"))], &["clip_id"]),
        run: |ss, a| { let id = str_arg(a, "clip_id")?; let mut found = false; for t in ss.project.tracks.iter_mut() { let l = t.clips.len(); t.clips.retain(|c| c.id != id); found |= t.clips.len() != l; } if found { Ok(json!({"ok":true})) } else { Err("no such clip".into()) } } },
    Command { name: "set_speed", description: "Set constant playback speed (0.1..100). Duration on the timeline is unchanged; source consumed = duration*speed.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("speed", n("multiplier"))], &["clip_id","speed"]),
        run: |ss, a| { let v = num_arg(a, "speed")?; if !(0.1..=100.0).contains(&v) { return Err("speed must be 0.1..100".into()); } clip(&mut ss.project, a)?.speed = v; Ok(json!({"ok":true})) } },
    Command { name: "set_property", description: "Set a base transform property: opacity (0..1), x, y (pixels), scale, rotation (degrees).", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("property", json!({"type":"string","enum":["opacity","x","y","scale","rotation"]})), ("value", n("value"))], &["clip_id","property","value"]),
        run: |ss, a| { let k = str_arg(a, "property")?.to_string(); let v = num_arg(a, "value")?; let c = clip(&mut ss.project, a)?; c.props.entry(k).or_insert(Prop::new(0.0)).base = v; Ok(json!({"ok":true})) } },
    Command { name: "set_keyframe", description: "Keyframe a transform property or an effect param (effect_id + property = param name) at clip-local time. ease: linear|in|out|in_out|hold.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("property", s("opacity|x|y|scale|rotation or effect param")), ("effect_id", s("optional: target an effect")), ("time", n("clip-local seconds")), ("value", n("value")), ("ease", json!({"type":"string","enum":["linear","in","out","in_out","hold"]}))], &["clip_id","property","time","value"]),
        run: |ss, a| { let k = str_arg(a, "property")?.to_string(); let t = num_arg(a, "time")?; let v = num_arg(a, "value")?;
            let e: Ease = serde_json::from_value(a.get("ease").cloned().unwrap_or(json!("linear"))).map_err(|e| e.to_string())?;
            let eid = a.get("effect_id").and_then(|v| v.as_str()).map(String::from);
            let c = clip(&mut ss.project, a)?;
            let pr = match eid { Some(id) => c.effects.iter_mut().find(|x| x.id == id).ok_or("no such effect")?.params.entry(k).or_insert(Prop::new(v)), None => c.props.entry(k).or_insert(Prop::new(v)) };
            pr.set_key(t, v, e); Ok(json!({"keys":pr.keys.len()})) } },
    Command { name: "list_effects", description: "List all available effects with parameters and defaults.", mutates: false,
        schema: obj(vec![], &[]),
        run: |_, _| Ok(json!(CATALOG.iter().chain(CATALOG2.iter()).chain(catalog3().iter()).map(|(k, d, ps)| json!({"kind":k,"description":d,"params":ps.iter().map(|(n,dv,dd)| json!({"name":n,"default":dv,"description":dd})).collect::<Vec<_>>()})).collect::<Vec<_>>())) },
    Command { name: "add_effect", description: "Add an effect to a clip (see list_effects). params is {name: number}. Effects run in order.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("kind", json!({"type":"string","enum":eff_kinds})), ("params", json!({"type":"object","additionalProperties":{"type":"number"}})), ("strings", json!({"type":"object","additionalProperties":{"type":"string"},"description":"string params, e.g. lut: {path:\"/x.cube\"}"}))], &["clip_id","kind"]),
        run: |ss, a| { let kind = str_arg(a, "kind")?.to_string(); if !CATALOG.iter().chain(CATALOG2.iter()).chain(catalog3().iter()).any(|c| c.0 == kind) { return Err(format!("unknown effect '{kind}'")); }
            let eid = ss.project.gen_id("fx"); let mut params = std::collections::BTreeMap::new();
            let mut strings = std::collections::BTreeMap::new();
            if let Some(o) = a.get("strings").and_then(|v| v.as_object()) { for (k, v) in o { strings.insert(k.clone(), v.as_str().unwrap_or("").to_string()); } }
            if let Some(o) = a.get("params").and_then(|v| v.as_object()) { for (k, v) in o { params.insert(k.clone(), Prop::new(v.as_f64().ok_or("param must be number")?)); } }
            clip(&mut ss.project, a)?.effects.push(Effect { id: eid.clone(), kind, params, strings }); Ok(json!({"effect_id":eid})) } },
    Command { name: "set_effect_param", description: "Set a base effect parameter value.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("effect_id", s("effect")), ("param", s("name")), ("value", n("value"))], &["clip_id","effect_id","param","value"]),
        run: |ss, a| { let eid = str_arg(a, "effect_id")?.to_string(); let k = str_arg(a, "param")?.to_string(); let v = num_arg(a, "value")?;
            let e = clip(&mut ss.project, a)?.effects.iter_mut().find(|x| x.id == eid).ok_or("no such effect")?; e.params.entry(k).or_insert(Prop::new(v)).base = v; Ok(json!({"ok":true})) } },
    Command { name: "remove_effect", description: "Remove an effect from a clip.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("effect_id", s("effect"))], &["clip_id","effect_id"]),
        run: |ss, a| { let eid = str_arg(a, "effect_id")?.to_string(); let c = clip(&mut ss.project, a)?; let l = c.effects.len(); c.effects.retain(|x| x.id != eid); if c.effects.len() == l { Err("no such effect".into()) } else { Ok(json!({"ok":true})) } } },
    Command { name: "add_transition", description: "Transition INTO a clip over its first `duration` seconds; overlap it with the previous clip on a lower/same-time layer. kinds: crossfade, wipe_left/right/up/down, diagonal, blinds, checker, iris, slide_left/right/up/down, zoom, spin, flash.", mutates: true,
        schema: obj(vec![("clip_id", s("incoming clip")), ("kind", json!({"type":"string","enum":["crossfade","wipe_left","wipe_up","wipe_right","wipe_down","diagonal","blinds","checker","iris","slide_left","slide_up","slide_right","slide_down","zoom","spin","flash"]})), ("duration", n("seconds"))], &["clip_id","kind","duration"]),
        run: |ss, a| { let k = str_arg(a, "kind")?.into(); let d = num_arg(a, "duration")?; clip(&mut ss.project, a)?.transition_in = Some(Transition { kind: k, duration: d }); Ok(json!({"ok":true})) } },
    Command { name: "set_blend", description: "Blend mode: normal, add, multiply, screen, overlay.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("mode", json!({"type":"string","enum":["normal","add","multiply","screen","overlay"]}))], &["clip_id","mode"]),
        run: |ss, a| { let m = str_arg(a, "mode")?.to_string(); clip(&mut ss.project, a)?.blend = m; Ok(json!({"ok":true})) } },
    Command { name: "get_project", description: "Return the full project JSON (tracks, clips, effects, keyframes).", mutates: false,
        schema: obj(vec![], &[]), run: |ss, _| serde_json::to_value(&ss.project).map_err(|e| e.to_string()) },
    Command { name: "render_frame", description: "Render the frame at a timeline time to a PNG file so you can look at it. Returns the path.", mutates: false,
        schema: obj(vec![("time", n("seconds")), ("path", s("output .png path"))], &["time","path"]),
        run: |ss, a| { let p = str_arg(a, "path")?; render::render_png(&ss.project, num_arg(a, "time")?, p, &mut ss.cache)?; Ok(json!({"path":p})) } },
    Command { name: "export", description: "Render the whole timeline to an H.264 mp4.", mutates: false,
        schema: obj(vec![("path", s("output .mp4 path")), ("crf", n("quality 0..51, default 18"))], &["path"]),
        run: |ss, a| { let p = str_arg(a, "path")?; let d = render::export(&ss.project, p, opt_num(a, "crf", 18.0) as u32)?; Ok(json!({"path":p,"duration":d})) } },
    Command { name: "save_project", description: "Save the project to a JSON file.", mutates: false,
        schema: obj(vec![("path", s("file path"))], &["path"]),
        run: |ss, a| { let p = str_arg(a, "path")?; std::fs::write(p, serde_json::to_string_pretty(&ss.project).unwrap()).map_err(|e| e.to_string())?; Ok(json!({"path":p})) } },
    Command { name: "load_project", description: "Load a project JSON file.", mutates: true,
        schema: obj(vec![("path", s("file path"))], &["path"]),
        run: |ss, a| { let t = std::fs::read_to_string(str_arg(a, "path")?).map_err(|e| e.to_string())?; ss.project = serde_json::from_str(&t).map_err(|e| e.to_string())?;
            ss.project.next_id = 1000; Ok(json!({"tracks":ss.project.tracks.len()})) } },
    Command { name: "undo", description: "Undo the last mutating operation.", mutates: false,
        schema: obj(vec![], &[]),
        run: |ss, _| { let prev = ss.undo.pop().ok_or("nothing to undo")?; let cur = std::mem::replace(&mut ss.project, prev); ss.redo.push(cur); Ok(json!({"ok":true})) } },
    Command { name: "redo", description: "Redo the last undone operation.", mutates: false,
        schema: obj(vec![], &[]),
        run: |ss, _| { let nx = ss.redo.pop().ok_or("nothing to redo")?; let cur = std::mem::replace(&mut ss.project, nx); ss.undo.push(cur); Ok(json!({"ok":true})) } },
    ]; v.extend(crate::edit::commands()); v.extend(crate::colortools::commands()); v
}

pub fn dispatch(ss: &mut Session, name: &str, args: &Value) -> Result<Value, String> {
    let reg = registry();
    let c = reg.iter().find(|c| c.name == name).ok_or(format!("unknown tool '{name}'"))?;
    let before = ss.project.clone();
    let r = (c.run)(ss, args);
    if c.mutates && r.is_ok() { ss.undo.push(before); ss.redo.clear(); }
    r
}
