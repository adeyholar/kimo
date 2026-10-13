//! U4 independent slice: failing fixtures first, no text-layout guess.
use kimo_core::commands::{dispatch, dispatch_ex, Session};
use serde_json::{json, Value};
fn c(s: &mut Session, n: &str, a: Value) -> Value {
    dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n}: {e}"))
}
fn scene() -> (Session, String) {
    let mut s = Session::new();
    c(
        &mut s,
        "project_set",
        json!({"width":160,"height":90,"fps":24}),
    );
    let t = c(&mut s, "add_track", json!({}))["track_id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = c(
        &mut s,
        "add_shape",
        json!({"track_id":t,"shape":"rect","width":40,"height":20,"start":0,"duration":6}),
    )["clip_id"]
        .as_str()
        .unwrap()
        .to_string();
    c(
        &mut s,
        "set_meta",
        json!({"id":id,"name":"plate","role":"foreground"}),
    );
    (s, id)
}
fn findings(r: &Value) -> Vec<&Value> {
    r["frames"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["findings"].as_array().unwrap())
        .collect()
}
#[test]
fn forty_pixels_outside_and_suppression_count() {
    let (mut s, id) = scene();
    c(
        &mut s,
        "set_property",
        json!({"clip_id":id,"property":"x","value":100}),
    );
    let r = c(
        &mut s,
        "lint_frame",
        json!({"time":1,"checks":["out_of_bounds"],"annotate":false}),
    );
    let f = findings(&r);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0]["measured"]["outside_px"]["right"], 40.);
    assert_eq!(f[0]["layer"]["name"], "plate");
    c(
        &mut s,
        "set_meta",
        json!({"id":id,"tags":["lint:allow_bleed"]}),
    );
    let r = c(
        &mut s,
        "lint_frame",
        json!({"time":1,"checks":["out_of_bounds"],"annotate":false}),
    );
    assert!(findings(&r).is_empty());
    assert_eq!(r["summary"]["ignored_by_tag"], 1);
}
#[test]
fn clean_zero_and_read_only() {
    let (mut s, _) = scene();
    let before = serde_json::to_vec(&s.project).unwrap();
    let h = c(&mut s, "history_list", json!({}));
    let r = c(&mut s, "lint_frame", json!({"time":1,"annotate":false}));
    assert!(findings(&r).is_empty(), "{r}");
    assert_eq!(before, serde_json::to_vec(&s.project).unwrap());
    assert_eq!(h, c(&mut s, "history_list", json!({})));
}
#[test]
fn zero_opacity_is_info_and_ignore_listed() {
    let (mut s, id) = scene();
    c(
        &mut s,
        "set_property",
        json!({"clip_id":id,"property":"opacity","value":0}),
    );
    let r = c(&mut s, "lint_frame", json!({"time":1,"annotate":false}));
    assert!(findings(&r)
        .iter()
        .any(|f| f["code"] == "invisible_layer" && f["severity"] == "info"));
    c(
        &mut s,
        "set_meta",
        json!({"id":id,"tags":["lint:ignore=invisible_layer"]}),
    );
    let r = c(&mut s, "lint_frame", json!({"time":1,"annotate":false}));
    assert_eq!(
        r["summary"]["suppressed"][0]["tag"],
        "lint:ignore=invisible_layer"
    );
}
#[test]
fn static_five_seconds_interval_and_override() {
    let (mut s, _) = scene();
    let r = c(
        &mut s,
        "lint_frame",
        json!({"range":{"start":0,"end":5,"step":0.5},"checks":["static_frame"],"annotate":false}),
    );
    assert_eq!(r["intervals"].as_array().unwrap().len(), 1);
    assert_eq!(r["intervals"][0]["t0"], 0.);
    assert_eq!(r["intervals"][0]["t1"], 5.);
    assert_eq!(r["intervals"][0]["worst"]["audio_present"], false);
    let r = c(
        &mut s,
        "lint_frame",
        json!({"range":{"start":0,"end":5},"checks":["static_frame"],"params":{"max_static_s":6},"annotate":false}),
    );
    assert!(r["intervals"].as_array().unwrap().is_empty());
    assert_eq!(r["params_used"]["max_static_s"], 6.);
}
#[test]
fn named_validation_errors() {
    let (mut s, _) = scene();
    for (a, code) in [
        (json!({"time":-1}), "out_of_range"),
        (json!({"time":1,"checks":["bogus"]}), "invalid_check"),
        (
            json!({"times":[0,1,2],"params":{"max_frames":2}}),
            "too_many_frames",
        ),
    ] {
        assert_eq!(
            dispatch_ex(&mut s, "lint_frame", &a).unwrap_err().code,
            code
        );
    }
}
#[test]
fn stable_across_threads() {
    let (mut s, id) = scene();
    c(
        &mut s,
        "set_property",
        json!({"clip_id":id,"property":"x","value":100}),
    );
    let a = json!({"times":[0,1,2],"checks":["out_of_bounds"],"annotate":false});
    kimo_core::par::set_threads(1);
    let mut x = c(&mut s, "lint_frame", a.clone());
    kimo_core::par::set_threads(2);
    let mut y = c(&mut s, "lint_frame", a);
    kimo_core::par::set_threads(0);
    for v in [&mut x, &mut y] {
        for f in v["frames"].as_array_mut().unwrap() {
            f.as_object_mut().unwrap().remove("elapsed_ms");
        }
    }
    assert_eq!(x, y);
}
#[test]
fn parent_scale_and_rotation_geometry() {
    let (mut s, id) = scene();
    let t = s.project.tracks[0].id.clone();
    let parent = c(
        &mut s,
        "add_null",
        json!({"track_id":t,"start":0,"duration":6,"scale":2,"rotation":90}),
    )["clip_id"]
        .as_str()
        .unwrap()
        .to_string();
    c(
        &mut s,
        "set_parent",
        json!({"clip_id":id,"parent_id":parent,"mode":"keep_local"}),
    );
    c(
        &mut s,
        "set_property",
        json!({"clip_id":id,"property":"x","value":50}),
    );
    let r = c(
        &mut s,
        "lint_frame",
        json!({"time":1,"checks":["out_of_bounds"],"annotate":false}),
    );
    let f = findings(&r);
    let hit = f.iter().find(|f| f["layer"]["id"] == id).unwrap();
    assert_eq!(hit["measured"]["outside_px"]["bottom"], 95.);
}
#[test]
fn annotations_write_image_and_limit_reports_omission() {
    let (mut s, id) = scene();
    c(
        &mut s,
        "set_property",
        json!({"clip_id":id,"property":"x","value":100}),
    );
    c(
        &mut s,
        "set_property",
        json!({"clip_id":id,"property":"opacity","value":0}),
    );
    let r = c(&mut s, "lint_frame", json!({"time":1,"max_findings":1}));
    assert_eq!(findings(&r).len(), 1);
    assert_eq!(r["summary"]["omitted_findings"], 1);
    let path = r["frames"][0]["annotation"]["path"].as_str().unwrap();
    assert!(std::fs::metadata(path).unwrap().len() > 0);
    assert_eq!(s.media.last().unwrap().path, path);
}
#[test]
fn adjacent_hits_merge_and_worst_is_largest_overflow() {
    let (mut s, id) = scene();
    c(
        &mut s,
        "set_keyframes",
        json!({"clip_id":id,"property":"x","keys":[{"t":0,"v":90},{"t":2,"v":110}]}),
    );
    let r = c(
        &mut s,
        "lint_frame",
        json!({"times":[0,1,2],"checks":["out_of_bounds"],"annotate":false}),
    );
    assert_eq!(r["intervals"].as_array().unwrap().len(), 1);
    assert_eq!(r["intervals"][0]["worst"]["outside_px"]["right"], 50.);
}
#[test]
fn malformed_params_and_random_times_never_panic() {
    let (mut s, _) = scene();
    for v in [
        json!(null),
        json!([]),
        json!("bad"),
        json!({"max_frames":1.5}),
        json!({"max_static_s":-1}),
        json!({"unknown":1}),
    ] {
        assert!(dispatch(
            &mut s,
            "lint_frame",
            &json!({"time":0,"params":v,"annotate":false})
        )
        .is_err());
    }
    let mut seed = 5u64;
    for _ in 0..200 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let t = (seed % 1600) as f64 / 100. - 5.;
        let a = json!({"time":t,"annotate":false,"params":{"opacity_epsilon":((seed>>32)%200)as f64/100.}});
        let _ = dispatch(&mut s, "lint_frame", &a);
    }
}
#[test]
fn checks_select_only_requested_and_cap_before_render() {
    let (mut s, id) = scene();
    c(
        &mut s,
        "set_property",
        json!({"clip_id":id,"property":"x","value":100}),
    );
    let r = c(
        &mut s,
        "lint_frame",
        json!({"times":[0,1],"checks":[],"annotate":false}),
    );
    assert!(findings(&r).is_empty());
    assert_eq!(r["summary"]["frames_checked"], 2);
    assert!(dispatch(
        &mut s,
        "lint_frame",
        &json!({"range":{"start":0,"end":6,"step":0.000001}})
    )
    .unwrap_err()
    .starts_with("too_many_frames:"));
    assert!(
        dispatch(&mut s, "lint_frame", &json!({"time":1,"profile":"social"}))
            .unwrap_err()
            .starts_with("out_of_range:")
    );
}
