//! U1 dispatch hook: "@name" is accepted wherever a clip_id / parent_id / track_id (or clip_ids) argument exists, at top level and inside batch ops.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};
fn c(ss: &mut Session, n: &str, v: Value) -> Value { dispatch(ss, n, &v).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn scene() -> (Session, String, String, String) {
    let mut ss = Session::new(); c(&mut ss, "project_set", json!({"width": 160, "height": 90, "fps": 25})); let t = c(&mut ss, "add_track", json!({}))["track_id"].as_str().unwrap().to_string();
    let a = c(&mut ss, "add_shape", json!({"track_id": t, "shape": "rect", "width": 20, "height": 20, "start": 0.0, "duration": 4.0}))["clip_id"].as_str().unwrap().to_string();
    let b = c(&mut ss, "add_shape", json!({"track_id": t, "shape": "rect", "width": 20, "height": 20, "start": 4.0, "duration": 4.0}))["clip_id"].as_str().unwrap().to_string();
    c(&mut ss, "set_meta", json!({"id": a, "name": "hero"})); c(&mut ss, "set_meta", json!({"target": "track", "id": t, "name": "main"})); (ss, t, a, b)
}
fn x_of(ss: &mut Session, id: &str) -> f64 { c(ss, "get_transform", json!({"clip_id": id, "t": 0.5, "space": "local"}))["position"]["x"].as_f64().or_else(|| c(ss, "get_transform", json!({"clip_id": id, "t": 0.5, "space": "local"}))["x"].as_f64()).unwrap_or(f64::NAN) }
#[test] fn at_name_works_for_clip_and_track_arguments_and_plain_ids_still_do() {
    let (mut ss, t, a, _) = scene();
    c(&mut ss, "set_property", json!({"clip_id": "@hero", "property": "x", "value": 12.0})); assert_eq!(ss.project.find_clip_mut(&a).unwrap().props["x"].at(0.0), 12.0);
    let n = c(&mut ss, "add_shape", json!({"track_id": "@main", "shape": "rect", "width": 5, "height": 5, "start": 8.0, "duration": 1.0})); assert!(n["clip_id"].is_string());
    assert!(ss.project.tracks.iter().find(|x| x.id == t).unwrap().clips.iter().any(|k| k.id == n["clip_id"].as_str().unwrap()));
    c(&mut ss, "set_property", json!({"clip_id": a, "property": "x", "value": 3.0})); assert_eq!(ss.project.find_clip_mut(&a).unwrap().props["x"].at(0.0), 3.0);
    let _ = x_of;
}
#[test] fn parent_id_and_batch_ops_resolve_names() {
    let (mut ss, _, a, b) = scene(); c(&mut ss, "set_meta", json!({"id": b, "name": "kid"}));
    c(&mut ss, "set_parent", json!({"clip_id": "@kid", "parent_id": "@hero"})); assert_eq!(ss.project.find_clip_mut(&b).unwrap().parent.as_deref(), Some(a.as_str()));
    let r = c(&mut ss, "batch", json!({"ops": [{"tool": "set_property", "args": {"clip_id": "@hero", "property": "y", "value": 7.0}}]})); assert!(r.to_string().contains("ok") || r["applied"].is_number() || r.is_object());
    assert_eq!(ss.project.find_clip_mut(&a).unwrap().props["y"].at(0.0), 7.0);
}
#[test] fn bad_names_fail_before_anything_runs_and_free_text_is_untouched() {
    let (mut ss, t, ..) = scene(); let before = serde_json::to_string(&ss.project).unwrap();
    let e = dispatch(&mut ss, "set_property", &json!({"clip_id": "@hreo", "property": "x", "value": 1.0})).unwrap_err(); assert!(e.contains("name_not_found") && e.contains("@hero"), "{e}");
    assert_eq!(before, serde_json::to_string(&ss.project).unwrap());
    let tx = c(&mut ss, "add_text", json!({"track_id": t, "text": "@hero", "start": 9.0, "duration": 1.0}));
    let id = tx["clip_id"].as_str().unwrap().to_string(); let txt = serde_json::to_string(&ss.project.find_clip_mut(&id).unwrap().source).unwrap(); assert!(txt.contains("@hero"), "text argument must not be resolved: {txt}");
}
