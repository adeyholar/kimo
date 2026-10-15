//! Spectator telemetry, end to end against the real kimo-mcp binary:
//! start/complete/error with matching call_id over a file sink and over the GUI's TCP transport,
//! and MCP stdout byte-identical with telemetry off, on, and pointed at a dead listener.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn requests() -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"project_set","arguments":{"width":320,"height":180,"fps":24}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"add_track","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"add_text","arguments":{"track_id":"track1","text":"kimo","start":0,"duration":1}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"set_property","arguments":{"clip_id":"clip2","property":"x","value":40}}}),
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"set_property","arguments":{"clip_id":"clip404","property":"x","value":1}}}),
        json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"get_project","arguments":{}}}),
    ]
}

/// Run kimo-mcp over `reqs` with KIMO_SPECTATOR = `spec` (None = unset). Returns stdout bytes.
fn run(spec: Option<&str>, reqs: &[Value]) -> Vec<u8> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kimo-mcp"));
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).env_remove("KIMO_SPECTATOR").env_remove("KIMO_TOOLS");
    if let Some(s) = spec { cmd.env("KIMO_SPECTATOR", s); }
    let mut ch = cmd.spawn().expect("spawn kimo-mcp");
    {
        let mut stdin = ch.stdin.take().unwrap();
        for r in reqs { writeln!(stdin, "{r}").unwrap(); }
    }
    let out = ch.wait_with_output().unwrap();
    assert!(out.status.success(), "kimo-mcp exited {:?}", out.status);
    out.stdout
}

fn events_of(text: &str) -> Vec<Value> { text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).expect("each telemetry line is JSON")).collect() }

fn tool_events<'a>(evs: &'a [Value], tool: &str) -> Vec<&'a Value> { evs.iter().filter(|e| e["tool_name"] == tool).collect() }

fn check_events(evs: &[Value]) {
    // every call: exactly one start then one terminal event with the same call_id
    let starts: Vec<&Value> = evs.iter().filter(|e| e["phase"] == "start").collect();
    assert_eq!(starts.len(), 7, "seven tools/call requests -> seven starts: {evs:#?}");
    for s in &starts {
        let id = s["call_id"].as_str().unwrap();
        let ends: Vec<&Value> = evs.iter().filter(|e| e["call_id"] == id && (e["phase"] == "complete" || e["phase"] == "error")).collect();
        assert_eq!(ends.len(), 1, "one terminal event for {id}");
        let e = ends[0];
        assert!(e["duration_ms"].as_f64().unwrap() >= 0.0);
        assert_eq!(e["payload"]["dur_ms"], e["duration_ms"]);
        assert!(e["ts_ms"].as_u64().unwrap() >= s["ts_ms"].as_u64().unwrap());
        assert!(e["ts"].as_f64().unwrap() >= s["ts"].as_f64().unwrap());
        // the GUI router treats any event with `type` as legacy, so tool events must not carry one
        assert!(s.get("type").is_none() && e.get("type").is_none());
        assert!(s["target"].is_object() && s["payload"]["args"].is_object());
    }
    assert_eq!(evs[0]["type"], "session", "a session reset leads the stream");
    for e in evs { assert!(e.get("tool_name").is_some() != e.get("type").is_some(), "tool event xor engine event: {e}"); }
    let ids: std::collections::HashSet<&str> = starts.iter().map(|s| s["call_id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 7, "call ids are unique");

    let at = tool_events(evs, "add_track");
    assert_eq!(at[1]["phase"], "complete");
    assert_eq!(at[1]["payload"]["result"]["track_id"], "track1");
    assert_eq!(at[1]["payload"]["changed"]["tracks_added"], json!(["track1"]));

    let tx = tool_events(evs, "add_text");
    assert_eq!(tx[0]["target"]["track_id"], "track1");
    assert_eq!(tx[0]["payload"]["args"]["text"], "kimo");
    assert_eq!(tx[1]["payload"]["changed"]["clips_added"][0]["clip_id"], "clip2");
    // router vocabulary for an insert
    assert_eq!(tx[1]["payload"]["clip"], "clip2"); assert_eq!(tx[1]["payload"]["track"], "track1");
    assert_eq!(tx[1]["payload"]["s"], 0.0); assert_eq!(tx[1]["payload"]["e"], 1.0); assert_eq!(tx[1]["payload"]["label"], "kimo");

    let sp = tool_events(evs, "set_property");
    assert_eq!(sp[0]["target"], json!({"track_id":"track1","clip_id":"clip2","param":"x"}), "track resolved from the project");
    assert_eq!(sp[1]["phase"], "complete");
    assert_eq!(sp[1]["payload"]["mutated"], true);
    assert!(sp[1]["payload"]["changed"]["clip"].as_object().map_or(false, |o| !o.is_empty()), "clip field diff: {:#}", sp[1]);
    assert_eq!((sp[1]["payload"]["from"].as_f64(), sp[1]["payload"]["to"].as_f64()), (Some(0.0), Some(40.0)), "from/to for the inspector: {:#}", sp[1]);

    let bad = tool_events(evs, "no_such_tool");
    assert_eq!(bad[1]["phase"], "error");
    assert!(bad[1]["payload"]["error"].as_str().unwrap().contains("no_such_tool"));
    assert_eq!(bad[1]["payload"]["code"], "unknown_tool");
    assert_eq!(bad[1]["call_id"], bad[0]["call_id"]);

    assert_eq!(sp[2]["phase"], "start"); assert_eq!(sp[3]["phase"], "error");
    assert_eq!(sp[3]["payload"]["code"], "not_found");

    let gp = tool_events(evs, "get_project");
    assert_eq!(gp[1]["payload"]["mutated"], false);
    assert_eq!(evs.iter().filter(|e| e["type"] == "project_changed").count(), 4, "project_set, add_track, add_text, set_property mutate");
}

fn tmp(name: &str) -> std::path::PathBuf { std::env::temp_dir().join(format!("kimo-spectator-{}-{name}", std::process::id())) }

#[test]
fn file_sink_start_complete_error() {
    let p = tmp("file.jsonl"); let _ = std::fs::remove_file(&p);
    run(Some(&format!("file:{}", p.display())), &requests());
    check_events(&events_of(&std::fs::read_to_string(&p).unwrap()));
    let _ = std::fs::remove_file(&p);
}

#[test]
fn tcp_sink_feeds_the_gui_transport() {
    // the GUI shell listens; the engine connects (crates/kimo-gui/src/main.rs)
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let rx = std::thread::spawn(move || {
        let (s, _) = l.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        BufReader::new(s).lines().map_while(Result::ok).collect::<Vec<_>>().join("\n")
    });
    run(Some(&format!("tcp://127.0.0.1:{port}")), &requests());
    check_events(&events_of(&rx.join().unwrap()));
}

#[test]
fn stdout_is_byte_identical_with_telemetry_off_on_or_unreachable() {
    let reqs = requests();
    let off = run(None, &reqs);
    let p = tmp("ident.jsonl");
    let on_file = run(Some(&format!("file:{}", p.display())), &reqs);
    let dead = { let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); l.local_addr().unwrap().port() }; // closed again: nobody listening
    let t = Instant::now();
    let on_dead = run(Some(&format!("tcp://127.0.0.1:{dead}")), &reqs);
    let dead_secs = t.elapsed().as_secs_f64();
    assert_eq!(String::from_utf8_lossy(&off), String::from_utf8_lossy(&on_file));
    assert_eq!(off, on_file, "file sink changes nothing on stdout");
    assert_eq!(off, on_dead, "dead listener changes nothing on stdout");
    assert!(dead_secs < 10.0, "no listener must not stall the server ({dead_secs:.1}s)");
    assert_eq!(String::from_utf8_lossy(&off).lines().count(), 8, "one JSON-RPC response per request, nothing else");
    assert!(std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false), "the sink did receive events");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn effect_param_change_is_a_from_to() {
    let p = tmp("fx.jsonl"); let _ = std::fs::remove_file(&p);
    let mut reqs = requests()[..4].to_vec();
    reqs.push(json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"add_effect","arguments":{"clip_id":"clip2","kind":"temperature","params":{"amount":0.0}}}}));
    reqs.push(json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"set_effect_param","arguments":{"clip_id":"clip2","effect_id":"fx3","param":"amount","value":0.35}}}));
    reqs.push(json!({"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"effects","arguments":{"action":"set_effect_param","args":{"clip_id":"clip2","effect_id":"fx3","param":"amount","value":0.5}}}}));
    run(Some(&format!("file:{}", p.display())), &reqs);
    let evs = events_of(&std::fs::read_to_string(&p).unwrap());
    let add = tool_events(&evs, "add_effect");
    assert_eq!(add[1]["payload"]["effect"], "temperature");
    assert_eq!(add[1]["payload"]["result"]["effect_id"], "fx3");
    let set = tool_events(&evs, "set_effect_param");
    assert_eq!(set.len(), 4, "direct call and group call both report under the resolved tool name");
    assert_eq!(set[1]["payload"]["from"], 0.0); assert_eq!(set[1]["payload"]["to"], 0.35);
    assert_eq!(set[1]["payload"]["changed"]["clip"]["effects[fx3].params.amount.base"], json!([0.0, 0.35]));
    assert_eq!(set[2]["group"], "effects");
    assert_eq!(set[3]["payload"]["from"], 0.35); assert_eq!(set[3]["payload"]["to"], 0.5);
    let _ = std::fs::remove_file(&p);
}

#[test]
fn render_frame_reaches_the_viewer() {
    let p = tmp("frame.jsonl"); let _ = std::fs::remove_file(&p);
    let png = tmp("frame.png");
    let mut reqs = requests()[..4].to_vec();
    reqs.push(json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"render_frame","arguments":{"time":0.5,"path": png.display().to_string(),"inline":"never"}}}));
    run(Some(&format!("file:{}", p.display())), &reqs);
    let evs = events_of(&std::fs::read_to_string(&p).unwrap());
    let f = evs.iter().find(|e| e["type"] == "frame_rendered").expect("frame_rendered after render_frame");
    let b = f["png_b64"].as_str().unwrap();
    assert!(b.starts_with("iVBORw0KGgo"), "a PNG, base64");
    assert_eq!(b.len(), std::fs::metadata(&png).unwrap().len().div_ceil(3) as usize * 4);
    let rf = tool_events(&evs, "render_frame");
    assert_eq!(f["call_id"], rf[0]["call_id"]);
    let _ = std::fs::remove_file(&p); let _ = std::fs::remove_file(&png);
}

#[test]
fn export_reports_progress() {
    if Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| !s.success()).unwrap_or(true) {
        eprintln!("ffmpeg not on PATH; skipping the export progress check");
        return;
    }
    let p = tmp("export.jsonl"); let _ = std::fs::remove_file(&p);
    let mp4 = tmp("export.mp4");
    let mut reqs = requests()[..4].to_vec();
    reqs.push(json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"export","arguments":{"path": mp4.display().to_string(),"audio":false}}}));
    let out = String::from_utf8(run(Some(&format!("file:{}", p.display())), &reqs)).unwrap();
    let evs = events_of(&std::fs::read_to_string(&p).unwrap());
    let ex = tool_events(&evs, "export");
    assert_eq!(ex.first().unwrap()["phase"], "start");
    assert_eq!(ex.last().unwrap()["phase"], "complete", "export failed? stdout: {out}");
    let prog: Vec<f64> = ex.iter().filter(|e| e["phase"] == "progress").map(|e| e["payload"]["progress"].as_f64().unwrap()).collect();
    assert!(ex.iter().filter(|e| e["phase"] == "progress").all(|e| (e["payload"]["pct"].as_f64().unwrap() - 100.0 * e["payload"]["progress"].as_f64().unwrap()).abs() < 0.06));
    assert!(!prog.is_empty(), "export emits progress events");
    assert!(prog.windows(2).all(|w| w[0] <= w[1]), "progress is monotonic: {prog:?}");
    assert!((prog.last().unwrap() - 1.0).abs() < 1e-9, "progress reaches 1.0: {prog:?}");
    assert!(ex.iter().all(|e| e["call_id"] == ex[0]["call_id"]));
    let _ = std::fs::remove_file(&p); let _ = std::fs::remove_file(&mp4);
}
