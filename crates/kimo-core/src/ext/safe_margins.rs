//! RV-28 C3: query_safe_margins - a data table of documented safe areas and estimated platform UI zones. Never enforcement.
use super::kit::*;
use crate::commands::Session;

pub const PROFILES: [&str; 7] = ["title_safe", "action_safe", "tiktok", "reels", "shorts", "youtube", "custom"];
const AS_OF: &str = "2026-10-13";
const EST: &str = "estimate of the platform UI overlay, not published by the platform; check the current app before relying on it";
/// (name, kind, [x, y, w, h] as fractions of the canvas, notes)
type Z = (&'static str, &'static str, [f64; 4], &'static str);
fn table(profile: &str) -> Option<(&'static str, Vec<Z>)> {
    Some(match profile {
        "title_safe" => ("SMPTE-style title safe: 90% of width and height, centred (5% inset each side)", vec![("title_safe", "safe", [0.05, 0.05, 0.90, 0.90], "keep titles and lower thirds inside")]),
        "action_safe" => ("SMPTE-style action safe: 93% of width and height, centred (3.5% inset each side)", vec![("action_safe", "safe", [0.035, 0.035, 0.93, 0.93], "keep important action inside")]),
        "tiktok" => (EST, vec![("safe", "safe", [0.06, 0.10, 0.76, 0.62], "estimated area clear of UI"), ("top_bar", "avoid", [0.0, 0.0, 1.0, 0.10], "search / following tabs"), ("caption_and_sound_bar", "avoid", [0.0, 0.72, 0.82, 0.28], "username, caption, sound ticker"), ("action_rail", "avoid", [0.84, 0.40, 0.16, 0.60], "like, comment, share, profile buttons")]),
        "reels" => (EST, vec![("safe", "safe", [0.06, 0.12, 0.78, 0.60], "estimated area clear of UI"), ("top_bar", "avoid", [0.0, 0.0, 1.0, 0.12], "camera and title bar"), ("caption_bar", "avoid", [0.0, 0.74, 0.84, 0.26], "account, caption, audio"), ("action_rail", "avoid", [0.86, 0.45, 0.14, 0.55], "like, comment, share buttons")]),
        "shorts" => (EST, vec![("safe", "safe", [0.06, 0.10, 0.78, 0.66], "estimated area clear of UI"), ("top_bar", "avoid", [0.0, 0.0, 1.0, 0.10], "search and menu"), ("caption_bar", "avoid", [0.0, 0.78, 0.82, 0.22], "channel, title, subscribe"), ("action_rail", "avoid", [0.86, 0.45, 0.14, 0.55], "like, dislike, comment, share buttons")]),
        "youtube" => (EST, vec![("safe", "safe", [0.05, 0.05, 0.90, 0.80], "estimated area clear of player controls"), ("player_controls", "avoid", [0.0, 0.88, 1.0, 0.12], "progress bar and controls when visible"), ("end_screen_corner", "avoid", [0.72, 0.60, 0.28, 0.28], "end-screen cards and watermark")]),
        _ => return None,
    })
}
fn num_field(a: &Value, k: &str) -> Result<Option<f64>, String> { match a.get(k) { None | Some(Value::Null) => Ok(None), Some(v) => v.as_f64().filter(|x| x.is_finite() && *x > 0.0 && *x <= 16384.0).map(Some).ok_or(format!("out_of_range: {k} must be a number > 0 and <= 16384")) } }
fn query_safe_margins(ss: &mut Session, a: &Value) -> Result<Value, String> {
    let profile = a.get("profile").and_then(|v| v.as_str()).unwrap_or("title_safe");
    if !PROFILES.contains(&profile) { return Err(format!("invalid_profile: '{profile}' is not allowed. Valid: {}", PROFILES.join(", "))); }
    let (pw, ph) = (ss.project.width as f64, ss.project.height as f64);
    let (mut w, mut h) = (num_field(a, "width")?, num_field(a, "height")?);
    if let Some(asp) = a.get("aspect").and_then(|v| v.as_str()) {
        let (x, y) = asp.split_once(':').and_then(|(x, y)| Some((x.trim().parse::<f64>().ok()?, y.trim().parse::<f64>().ok()?))).filter(|(x, y)| *x > 0.0 && *y > 0.0 && x.is_finite() && y.is_finite()).ok_or("out_of_range: aspect must look like 16:9")?;
        match (w, h) { (Some(_), Some(_)) => return Err("out_of_range: give aspect with at most one of width, height".into()), (Some(wv), None) => h = Some((wv * y / x).round()), (None, Some(hv)) => w = Some((hv * x / y).round()), (None, None) => { w = Some(pw); h = Some((pw * y / x).round()); } }
    }
    let (w, h) = (w.unwrap_or(pw), h.unwrap_or(ph));
    let r = |f: [f64; 4]| json!([(f[0] * w * 100.0).round() / 100.0, (f[1] * h * 100.0).round() / 100.0, (f[2] * w * 100.0).round() / 100.0, (f[3] * h * 100.0).round() / 100.0]);
    let mut zones = vec![]; let (note, source);
    if profile == "custom" {
        let zs = a.get("zones").and_then(|v| v.as_array()).filter(|z| !z.is_empty() && z.len() <= 64).ok_or("out_of_range: profile custom needs zones: 1..64 of {name, rect:[x,y,w,h] pixels, kind: safe|avoid, notes?}")?;
        for (i, z) in zs.iter().enumerate() {
            let name = z.get("name").and_then(|v| v.as_str()).filter(|n| !n.is_empty() && n.len() <= 64).ok_or(format!("out_of_range: zones[{i}].name must be 1..64 characters"))?;
            let kind = z.get("kind").and_then(|v| v.as_str()).unwrap_or("avoid"); if !["safe", "avoid"].contains(&kind) { return Err(format!("out_of_range: zones[{i}].kind is not allowed. Valid: safe, avoid")); }
            let rc: Vec<f64> = z.get("rect").and_then(|v| v.as_array()).filter(|v| v.len() == 4).map(|v| v.iter().filter_map(|x| x.as_f64().filter(|f| f.is_finite())).collect()).filter(|v: &Vec<f64>| v.len() == 4).ok_or(format!("out_of_range: zones[{i}].rect must be [x,y,w,h] pixels"))?;
            if rc[2] <= 0.0 || rc[3] <= 0.0 || rc[0] < 0.0 || rc[1] < 0.0 || rc[0] + rc[2] > w + 1e-6 || rc[1] + rc[3] > h + 1e-6 { return Err(format!("out_of_range: zones[{i}].rect must lie inside the {w}x{h} canvas")); }
            zones.push(json!({"name": name, "rect": rc, "kind": kind, "notes": z.get("notes").and_then(|v| v.as_str()).unwrap_or("")}));
        }
        note = "user-supplied zones, echoed and validated"; source = "user";
    } else {
        let (n, t) = table(profile).unwrap();
        for (name, kind, f, notes) in t { zones.push(json!({"name": name, "rect": r(f), "kind": kind, "notes": notes})); }
        note = n; source = "documented-constant";
    }
    let est = !matches!(profile, "title_safe" | "action_safe" | "custom");
    Ok(json!({"profile": profile, "zones": zones, "source": source, "estimated": est, "as_of": AS_OF, "basis": note, "canvas": {"width": w, "height": h}, "read_only": "nothing is moved or rejected; these are numbers to compare against"}))
}
pub fn commands() -> Vec<Command> { vec![
    Command { name: "query_safe_margins", mutates: false, description: "Read-only, nothing is moved and nothing is rejected: return safe and avoid zones as pixel rects [x,y,w,h] for a canvas. profile title_safe (default, 90% of the frame) or action_safe (93%), the platform overlays tiktok, reels, shorts, youtube (labelled estimates with an as_of date, never enforcement), or custom with your own zones (validated and echoed). Canvas is the project size, or width/height, or aspect like 9:16 with one of width/height.",
        schema: obj(vec![("profile", json!({"type":"string","enum":PROFILES})), ("width", json!({"type":"number"})), ("height", json!({"type":"number"})), ("aspect", s("W:H such as 16:9, 9:16, 1:1")), ("zones", json!({"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"rect":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4},"kind":{"type":"string","enum":["safe","avoid"]},"notes":{"type":"string"}},"required":["name","rect"]}}))], &[]), run: query_safe_margins },
] }
