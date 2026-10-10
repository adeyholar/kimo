//! Command registry: single source of truth for every editor operation.
//! The MCP server lists and dispatches straight from `registry()`.
use crate::effects::CATALOG;
use crate::keyframe::{Ease, Prop};
use crate::model::*;
use crate::render;
use serde_json::{json, Value};

pub struct Session { pub project: Project, pub undo: Vec<Project>, pub redo: Vec<Project> }
impl Session { pub fn new() -> Self { Session { project: Project::default(), undo: vec![], redo: vec![] } } }

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
    let eff_kinds: Vec<&str> = CATALOG.iter().map(|c| c.0).collect();
    vec![
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
        run: |ss, _| { let id = ss.project.gen_id("track"); ss.project.tracks.push(Track { id: id.clone(), clips: vec![] }); Ok(json!({"track_id":id})) } },
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
            let c = Clip { id: id.clone(), source: src, start: num_arg(a, "start")?, duration: num_arg(a, "duration")?, in_point: opt_num(a, "in_point", 0.0), speed: opt_num(a, "speed", 1.0), props: default_props(), effects: vec![], blend: "normal".into(), mask: None, transition_in: None };
            let tr = ss.project.tracks.iter_mut().find(|t| t.id == tid).ok_or(format!("no track '{tid}'"))?; tr.clips.push(c);
            Ok(json!({"clip_id":id})) } },
    Command { name: "add_text", description: "Add a text clip (rendered with a TTF/OTF font). Animate with set_keyframe on reveal (typewriter), opacity, scale, x, y, rotation. Default font is a system sans.", mutates: true,
        schema: obj(vec![("track_id", s("track")), ("text", s("text, \\n for newlines")), ("start", n("seconds")), ("duration", n("seconds")), ("font", s("path to .ttf/.otf (optional)")), ("size", n("pixels, default 72")),
            ("color", json!({"type":"array","items":{"type":"number"}})), ("letter_spacing", n("pixels")), ("stroke_width", n("pixels")), ("stroke_color", json!({"type":"array","items":{"type":"number"}})), ("shadow", n("shadow offset pixels")), ("align", json!({"type":"string","enum":["left","center","right"]}))], &["track_id","text","start","duration"]),
        run: |ss, a| { let tid = str_arg(a, "track_id")?.to_string(); let id = ss.project.gen_id("clip");
            let src = Source::Text { text: str_arg(a, "text")?.replace("\\n", "\n"), font: a.get("font").and_then(|v| v.as_str()).unwrap_or("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf").into(), size: opt_num(a, "size", 72.0), color: color(a, "color", [1.0; 3]),
                letter_spacing: opt_num(a, "letter_spacing", 0.0), stroke_width: opt_num(a, "stroke_width", 0.0), stroke_color: color(a, "stroke_color", [0.0; 3]), shadow: opt_num(a, "shadow", 0.0), align: a.get("align").and_then(|v| v.as_str()).unwrap_or("center").into() };
            let c = Clip { id: id.clone(), source: src, start: num_arg(a, "start")?, duration: num_arg(a, "duration")?, in_point: 0.0, speed: 1.0, props: default_props(), effects: vec![], blend: "normal".into(), mask: None, transition_in: None };
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
        run: |_, _| Ok(json!(CATALOG.iter().map(|(k, d, ps)| json!({"kind":k,"description":d,"params":ps.iter().map(|(n,dv,dd)| json!({"name":n,"default":dv,"description":dd})).collect::<Vec<_>>()})).collect::<Vec<_>>())) },
    Command { name: "add_effect", description: "Add an effect to a clip (see list_effects). params is {name: number}. Effects run in order.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("kind", json!({"type":"string","enum":eff_kinds})), ("params", json!({"type":"object","additionalProperties":{"type":"number"}})), ("strings", json!({"type":"object","additionalProperties":{"type":"string"},"description":"string params, e.g. lut: {path:\"/x.cube\"}"}))], &["clip_id","kind"]),
        run: |ss, a| { let kind = str_arg(a, "kind")?.to_string(); if !CATALOG.iter().any(|c| c.0 == kind) { return Err(format!("unknown effect '{kind}'")); }
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
    Command { name: "add_transition", description: "Transition INTO a clip over its first `duration` seconds; overlap it with the previous clip on a lower/same-time layer. kinds: crossfade, wipe_left, wipe_up, iris, slide_left, slide_up, zoom.", mutates: true,
        schema: obj(vec![("clip_id", s("incoming clip")), ("kind", json!({"type":"string","enum":["crossfade","wipe_left","wipe_up","iris","slide_left","slide_up","zoom"]})), ("duration", n("seconds"))], &["clip_id","kind","duration"]),
        run: |ss, a| { let k = str_arg(a, "kind")?.into(); let d = num_arg(a, "duration")?; clip(&mut ss.project, a)?.transition_in = Some(Transition { kind: k, duration: d }); Ok(json!({"ok":true})) } },
    Command { name: "set_blend", description: "Blend mode: normal, add, multiply, screen, overlay.", mutates: true,
        schema: obj(vec![("clip_id", s("clip")), ("mode", json!({"type":"string","enum":["normal","add","multiply","screen","overlay"]}))], &["clip_id","mode"]),
        run: |ss, a| { let m = str_arg(a, "mode")?.to_string(); clip(&mut ss.project, a)?.blend = m; Ok(json!({"ok":true})) } },
    Command { name: "get_project", description: "Return the full project JSON (tracks, clips, effects, keyframes).", mutates: false,
        schema: obj(vec![], &[]), run: |ss, _| serde_json::to_value(&ss.project).map_err(|e| e.to_string()) },
    Command { name: "render_frame", description: "Render the frame at a timeline time to a PNG file so you can look at it. Returns the path.", mutates: false,
        schema: obj(vec![("time", n("seconds")), ("path", s("output .png path"))], &["time","path"]),
        run: |ss, a| { let p = str_arg(a, "path")?; render::render_png(&ss.project, num_arg(a, "time")?, p)?; Ok(json!({"path":p})) } },
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
    ]
}

pub fn dispatch(ss: &mut Session, name: &str, args: &Value) -> Result<Value, String> {
    let reg = registry();
    let c = reg.iter().find(|c| c.name == name).ok_or(format!("unknown tool '{name}'"))?;
    let before = ss.project.clone();
    let r = (c.run)(ss, args);
    if c.mutates && r.is_ok() { ss.undo.push(before); ss.redo.clear(); }
    r
}
