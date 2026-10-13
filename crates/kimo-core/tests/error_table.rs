//! Unit 5 baseline: ~40 deliberately bad calls and the EXACT error text each returns today. The baseline lives in tests/golden/error_texts.txt and must stay
//! byte-identical through the structured-error work (KIMO_UPDATE_GOLDEN=1 rewrites it). Ids are fixed by a deterministic setup (track1/track2, one shape clip).
use kimo_core::commands::{dispatch, dispatch_ex, Session};
use serde_json::{json, Value};

fn setup() -> (Session, String, String, String) {
    let mut ss = Session::new(); let c = |ss: &mut Session, n: &str, a: Value| dispatch(ss, n, &a).unwrap();
    c(&mut ss, "project_set", json!({"width": 320, "height": 180, "fps": 25}));
    let t1 = c(&mut ss, "add_track", json!({}))["track_id"].as_str().unwrap().to_string(); let t2 = c(&mut ss, "add_track", json!({}))["track_id"].as_str().unwrap().to_string();
    let cl = c(&mut ss, "add_shape", json!({"track_id": t1, "shape": "rect", "width": 60, "height": 40, "fill": [1, 0.5, 0, 1], "start": 1.0, "duration": 2.0}))["clip_id"].as_str().unwrap().to_string();
    (ss, t1, t2, cl)
}
fn calls(t1: &str, c: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("no_such_tool", json!({})), ("set_propertyy", json!({})),
        ("delete_clip", json!({})), ("delete_clip", json!({"clip_id": 5})), ("delete_clip", json!({"clip_id": "nope"})),
        ("move_clip", json!({"clip_id": c})), ("move_clip", json!({"clip_id": c, "start": "x"})), ("move_clip", json!({"clip_id": "nope", "start": 1.0})),
        ("trim_clip", json!({"clip_id": c, "duration": -1.0})), ("set_speed", json!({"clip_id": c, "speed": 0.0})), ("set_speed", json!({"clip_id": c, "speed": 1000.0})),
        ("set_property", json!({"clip_id": c, "property": "bogus", "value": 1.0})), ("set_property", json!({"clip_id": c, "property": "opacity"})),
        ("split_clip", json!({"clip_id": c, "at": 99.0})), ("split_clip", json!({"clip_id": c})),
        ("freeze_frame", json!({"clip_id": c, "at": 1.0, "hold_from_here": true})), ("freeze_frame", json!({"clip_id": c, "at": 99.0})),
        ("insert_gap", json!({"track_id": t1, "at": -1.0, "duration": 1.0})), ("insert_gap", json!({"track_id": t1, "at": 1.0, "duration": -1.0})), ("insert_gap", json!({"track_id": "nope", "at": 1.0, "duration": 1.0})),
        ("add_track", json!({"kind": 5})), ("add_shape", json!({"track_id": t1})), ("add_shape", json!({"track_id": "nope", "shape": "rect", "width": 1, "height": 1, "fill": [1, 1, 1, 1], "start": 0.0, "duration": 1.0})),
        ("add_shape", json!({"track_id": t1, "shape": "hexagonal", "width": 1, "height": 1, "fill": [1, 1, 1, 1], "start": 0.0, "duration": 1.0})),
        ("add_effect", json!({"clip_id": c, "kind": "no_such_effect"})), ("add_effect", json!({"clip_id": "nope", "kind": "blur"})),
        ("set_keyframe", json!({"clip_id": c, "property": "opacity", "time": 0.5})), ("set_keyframe", json!({"clip_id": c, "property": "bogus", "time": 0.5, "value": 1.0})),
        ("set_camera", json!({"cam_fov": 3.0})), ("set_camera", json!({"cam_x": "a"})),
        ("set_track", json!({"track_id": "nope", "locked": true})), ("remove_silence_cuts", json!({"track_id": "nope", "ranges": [[1.0, 2.0]]})),
        ("loop_clip", json!({"clip_id": c, "times": 0})), ("loop_clip", json!({"clip_id": c, "times": 1, "gap": -1.0})), ("swap_clips", json!({"clip_a": c, "clip_b": c})),
        ("distribute_clips", json!({"track_id": t1, "gap": -1.0})), ("get_scopes", json!({})), ("render_frame", json!({"time": "x"})),
        ("bind_shared_grade_graphs", json!({"clip_id": c, "graphs": ["missing"]})), ("save_project", json!({})), ("load_project", json!({"path": "/nonexistent/x.kimo"})), ("set_property", json!({"clip_id": c, "property": "x", "value": 1.0, "time": 0.0})), ("delete_clip", json!({"clipid": "x"})),
        ("cache_set_budget", json!({})), ("cache_set_budget", json!({"megabytes": -1.0})),
        ("collect_project", json!({})), ("collect_project", json!({"dest_dir": "/tmp"})), ("collect_project", json!({"dest_dir": "/tmp/kimo_err_row", "copy": "zip"})), ("relink_media", json!({"search_dirs": ["/tmp"], "match": "fuzzy"})),
        ("transcript_import", json!({})), ("transcript_import", json!({"path": "/nonexistent/x.srt"})), ("transcript_get", json!({"transcript_id": "nope"})), ("transcript_coverage", json!({})), ("transcript_to_captions", json!({"track_id": "nope"})), ("validate_captions", json!({})), ("transcript_edit_preview", json!({"edits": [{"op": "wipe", "from_index": 0}]})),
    ]
}
#[test] fn error_texts_are_byte_identical_to_the_baseline() {
    let (probe, t1, _t2, c) = setup(); drop(probe);
    let calls = calls(&t1, &c);
    let mut lines = vec![];
    for (i, (tool, args)) in calls.iter().enumerate() {
        let (mut ss, _, _, _) = setup(); let line = match dispatch(&mut ss, tool, args) { Ok(_) => "OK (no error)".to_string(), Err(e) => e.replace('\n', "\\n") };
        lines.push(format!("{i:02} {tool} {}\t{line}", serde_json::to_string(args).unwrap()));
    }
    let text = lines.join("\n") + "\n"; let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/error_texts.txt");
    if std::env::var("KIMO_UPDATE_GOLDEN").map(|v| v == "1").unwrap_or(false) { std::fs::write(&p, &text).unwrap(); return; }
    let want = std::fs::read_to_string(&p).expect("run once with KIMO_UPDATE_GOLDEN=1 to record the baseline"); assert_eq!(text, want, "error text changed");
}

#[test] fn every_baseline_row_has_the_expected_code_param_and_hint() {
    let (probe, t1, _t2, c) = setup(); drop(probe); let mut lines = vec![]; let mut other = 0; let mut n = 0;
    for (i, (tool, args)) in calls(&t1, &c).iter().enumerate() {
        let (mut ss, _, _, _) = setup(); let (mut ss2, _, _, _) = setup();
        match dispatch_ex(&mut ss, tool, args) { Ok(_) => lines.push(format!("{i:02} {tool}\tOK")), Err(e) => { n += 1; if e.code == "other" { other += 1; }
            assert_eq!(Some(e.message.clone()), dispatch(&mut ss2, tool, args).err(), "dispatch_ex text must equal dispatch text");
            lines.push(format!("{i:02} {tool}\t{}\tparam={}\thint={}", e.code, e.param.clone().unwrap_or_default(), e.hint.clone().unwrap_or_default())); } }
    }
    eprintln!("classified as other: {other} of {n}");
    let text = lines.join("\n") + "\n"; let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/error_codes.txt");
    if std::env::var("KIMO_UPDATE_GOLDEN").map(|v| v == "1").unwrap_or(false) { std::fs::write(&p, &text).unwrap(); return; }
    assert_eq!(text, std::fs::read_to_string(&p).expect("record with KIMO_UPDATE_GOLDEN=1"), "codes changed");
}
