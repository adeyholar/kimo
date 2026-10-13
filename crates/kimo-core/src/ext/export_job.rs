//! RV-6: chunked export job with status, cancel, resume and a safe incremental reuse rule.
//! A job renders picture-only chunks (`chunk_00000.mp4`, ...) next to `manifest.json` and `index.json`, then joins them with ONE continuous audio
//! stream (render::join_export). A chunk is reused only when its dependency key (export_manifest) matches the key recorded when it was rendered AND the
//! chunk file's sha256 still matches; duration and frame count are never freshness evidence. `force: true` re-renders everything and says so.
use super::export_manifest as em;
use super::kit::*;
use crate::deps;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

struct State {
    state: &'static str, frames_done: u64, frames_total: u64, started: std::time::Instant, current_range: Option<[u64; 2]>,
    chunks_done: u64, chunks_total: u64, chunks_reused: u64, warnings: Vec<Value>, rendered_secs: Vec<f64>, to_render: u64,
    completed: Vec<[u64; 2]>, error: Option<String>, result: Option<Value>, chunks_at: String, out: String,
}
struct Job { st: Mutex<State>, cancel: crate::guard::Cancel }
fn jobs() -> &'static Mutex<BTreeMap<String, Arc<Job>>> { static J: std::sync::OnceLock<Mutex<BTreeMap<String, Arc<Job>>>> = std::sync::OnceLock::new(); J.get_or_init(Default::default) }
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// ETA from measured chunk render times: null until `MIN_MEASURED` chunks were rendered, never false precision.
pub const MIN_MEASURED: usize = 3;
pub fn eta(measured: &[f64], remaining: u64) -> Value {
    if measured.len() < MIN_MEASURED { return Value::Null; }
    let n = measured.len() as f64; let mean = measured.iter().sum::<f64>() / n; let var = measured.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n; let cv = if mean > 0.0 { var.sqrt() / mean } else { 1.0 };
    json!({"seconds": mean * remaining as f64, "uncertainty": if cv < 0.15 { "low" } else if cv < 0.4 { "medium" } else { "high" }})
}

fn write_atomic(path: &std::path::Path, v: &Value) -> Result<(), String> { let tmp = path.with_extension("tmp"); std::fs::write(&tmp, serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?; std::fs::rename(&tmp, path).map_err(|e| e.to_string()) }
fn read_json(path: &std::path::Path) -> Option<Value> { serde_json::from_slice(&std::fs::read(path).ok()?).ok() }
fn file_sha(p: &std::path::Path) -> Option<String> { deps::hash_file(p, true).ok().map(|x| x.0) }
fn chunk_name(i: u64) -> String { format!("chunk_{i:05}.mp4") }

fn status_json(j: &Job) -> Value {
    let s = j.st.lock().unwrap_or_else(|e| e.into_inner()); let remaining = s.to_render.saturating_sub(s.rendered_secs.len() as u64);
    json!({"state": s.state, "frames_done": s.frames_done, "frames_total": s.frames_total, "elapsed_s": s.started.elapsed().as_secs_f64(), "current_range": s.current_range,
        "chunks": {"done": s.chunks_done, "total": s.chunks_total, "reused": s.chunks_reused}, "warnings": s.warnings, "eta": if s.state == "running" { eta(&s.rendered_secs, remaining) } else { Value::Null },
        "completed_ranges": s.completed, "dir": s.chunks_at, "path": s.out, "error": s.error, "result": s.result})
}
fn set<F: FnOnce(&mut State)>(j: &Job, f: F) { f(&mut j.st.lock().unwrap_or_else(|e| e.into_inner())); }

/// Why a chunk must be rendered, or None when the recorded one is still valid.
fn reuse_check(dir: &std::path::Path, idx: u64, key: &str, crf: u32, old: &Option<Value>, force: bool) -> Option<&'static str> {
    if force { return Some("forced"); }
    let Some(o) = old.as_ref().and_then(|o| o["chunks"].get(idx.to_string())) else { return Some("no_previous") };
    if o["key"] != json!(key) || o["crf"] != json!(crf) { return Some("key_changed"); }
    let f = dir.join(chunk_name(idx)); if !f.is_file() { return Some("file_missing"); }
    if file_sha(&f).as_deref() != o["sha256"].as_str() { return Some("file_hash_mismatch"); }
    None
}

fn run(j: Arc<Job>, p: crate::model::Project, dir: std::path::PathBuf, out: String, cf: u64, crf: u32, force: bool) {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Value, String> {
        let fps = if p.fps > 0.0 { p.fps } else { 30.0 }; let total = (em::project_end(&p) * fps).ceil() as u64; if total == 0 { return Err("nothing to export: the timeline is empty".into()); }
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let m = em::build(&p, 0, total, cf, false); write_atomic(&dir.join("manifest.json"), &m)?;
        let mut old = read_json(&dir.join("index.json")); let chunks = m["chunks"].as_array().cloned().unwrap_or_default();
        let plan: Vec<Option<&'static str>> = chunks.iter().map(|c| reuse_check(&dir, c["index"].as_u64().unwrap_or(0), c["key"].as_str().unwrap_or(""), crf, &old, force)).collect();
        set(&j, |s| { s.state = "running"; s.frames_total = total; s.chunks_total = chunks.len() as u64; s.to_render = plan.iter().filter(|x| x.is_some()).count() as u64; });
        let mut index = json!({"project_hash": m["project_hash"], "chunk_frames": cf, "chunks": {}}); let (mut reused, mut rendered): (Vec<u64>, Vec<Value>) = (vec![], vec![]);
        for (c, why) in chunks.iter().zip(plan.iter()) {
            let idx = c["index"].as_u64().unwrap_or(0); let range = [c["range"][0].as_u64().unwrap_or(0), c["range"][1].as_u64().unwrap_or(0)]; let file = dir.join(chunk_name(idx));
            if j.cancel.load(std::sync::atomic::Ordering::Relaxed) { return Err("cancelled".into()); }
            set(&j, |s| s.current_range = Some(range));
            let entry = match why {
                None => { reused.push(idx); old.as_ref().and_then(|o| o["chunks"].get(idx.to_string()).cloned()).unwrap_or(Value::Null) }
                Some(why) => {
                    let t0 = std::time::Instant::now(); let part = dir.join(format!("chunk_{idx:05}.partial.mp4")); let _ = std::fs::remove_file(&file);
                    let r = crate::render::export_range_opts(&p, part.to_str().unwrap_or(""), crf, range[0] as f64 / fps, range[1] as f64 / fps, true, false);
                    match r { Ok(_) => std::fs::rename(&part, &file).map_err(|e| e.to_string())?,
                        Err(e) => { let pf = dir.join(format!("{}.partial", chunk_name(idx))); let _ = std::fs::rename(&part, &pf); let _ = write_atomic(&dir.join(format!("{}.partial.marker", chunk_name(idx))), &json!({"range": range, "key": c["key"], "error": e})); return Err(e); } }
                    rendered.push(json!({"index": idx, "reason": why})); set(&j, |s| s.rendered_secs.push(t0.elapsed().as_secs_f64()));
                    json!({"key": c["key"], "range": range, "crf": crf, "sha256": file_sha(&file), "bytes": std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0)}) }
            };
            index["chunks"][idx.to_string()] = entry; write_atomic(&dir.join("index.json"), &index)?;
            set(&j, |s| { s.chunks_done += 1; s.frames_done += range[1] - range[0]; if why.is_none() { s.chunks_reused += 1; } s.completed.push(range); });
        }
        let parts: Vec<String> = (0..chunks.len() as u64).map(|i| dir.join(chunk_name(i)).to_string_lossy().into_owned()).collect();
        let dur = crate::render::join_export(&p, &parts, &out, 0.0)?;
        Ok(json!({"path": out, "duration": dur, "reused": reused, "rendered": rendered, "project_hash": m["project_hash"], "forced": force}))
    }));
    let cancelled = j.cancel.load(std::sync::atomic::Ordering::Relaxed);
    if !matches!(res, Ok(Ok(_))) { if let Ok(rd) = std::fs::read_dir(&dir) { for e in rd.flatten() { let n = e.file_name().to_string_lossy().into_owned(); if let Some(stem) = n.strip_suffix(".partial.mp4") { let pf = dir.join(format!("{stem}.mp4.partial")); let _ = std::fs::rename(e.path(), &pf); let _ = write_atomic(&dir.join(format!("{stem}.mp4.partial.marker")), &json!({"error": "interrupted before the chunk finished", "cancelled": cancelled})); } } } }
    set(&j, |s| { s.current_range = None; match res { Ok(Ok(v)) => { s.state = "done"; s.result = Some(v); }
        Ok(Err(e)) => { if cancelled || e == "cancelled" { s.state = "cancelled"; } else { s.state = "failed"; s.error = Some(e); } }
        Err(_) => { if cancelled { s.state = "cancelled"; } else { s.state = "failed"; s.error = Some("export job panicked".into()); } } } });
}

fn start(ss: &Session, a: &Value, resume: bool) -> Result<Value, String> {
    let out = str_arg(a, "path")?.to_string(); let dir = std::path::PathBuf::from(a.get("dir").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| format!("{out}.chunks")));
    let fps = if ss.project.fps > 0.0 { ss.project.fps } else { 30.0 }; let crf = opt_num(a, "crf", 18.0); if !(0.0..=51.0).contains(&crf) { return Err("crf must be 0..51".into()); }
    let mut cs = opt_num(a, "chunk_s", 4.0); let prev = read_json(&dir.join("manifest.json"));
    if resume { let m = prev.as_ref().ok_or_else(|| format!("resume_mismatch: no manifest.json in {} (nothing to resume; use export_job_start)", dir.display()))?;
        if a.get("chunk_s").is_none() { cs = m["settings"]["chunk_frames"].as_f64().unwrap_or(0.0) / fps; }
        let total = (em::project_end(&ss.project) * fps).ceil() as u64; let cur = em::build(&ss.project, 0, total, ((cs * fps).round() as u64).max(1), false);
        if cur["project_hash"] != m["project_hash"] || cur["settings"]["hash"] != m["settings"]["hash"] {
            let ex = em::explain(m, &cur); let first = ex["chunks"].as_array().and_then(|c| c.iter().find(|x| x["stale"] == true)).map(|c| c["reason"][0].clone()).unwrap_or(Value::Null);
            return Err(format!("resume_mismatch: the project changed since this job was started; first differing dependency: {first}. Start a new job (export_job_start) to reuse the chunks that are still valid.")); } }
    if !cs.is_finite() || cs <= 0.0 { return Err("chunk_s must be a positive number of seconds".into()); }
    let cf = ((cs * fps).round() as u64).max(1); let force = a.get("force").and_then(|v| v.as_bool()).unwrap_or(false); let wait = a.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let id = format!("job{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let j = Arc::new(Job { st: Mutex::new(State { state: "queued", frames_done: 0, frames_total: 0, started: std::time::Instant::now(), current_range: None, chunks_done: 0, chunks_total: 0, chunks_reused: 0, warnings: vec![], rendered_secs: vec![], to_render: 0, completed: vec![], error: None, result: None, chunks_at: dir.to_string_lossy().into_owned(), out: out.clone() }), cancel: crate::guard::new_token() });
    jobs().lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), j.clone());
    let (p, j2, tok) = (ss.project.clone(), j.clone(), j.cancel.clone());
    let h = crate::guard::spawn_with_token(move || crate::guard::with_token(&tok, || run(j2, p, dir, out, cf, crf as u32, force)));
    if wait { let _ = h.join(); }
    Ok(json!({"job_id": id, "status": status_json(&j)}))
}
fn get(a: &Value) -> Result<Arc<Job>, String> { let id = str_arg(a, "job_id")?; jobs().lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned().ok_or_else(|| format!("no such job '{id}'")) }

pub fn commands() -> Vec<Command> { vec![
    Command { name: "export_job_start", mutates: false,
        description: "Start a chunked export in the background and return a job_id (poll with job_status). Picture-only chunks are rendered into <path>.chunks/ with a manifest.json and index.json, then joined with ONE continuous audio stream into path. Incremental: a chunk is reused only when its dependency key matches AND the chunk file's sha256 still verifies; otherwise it is rendered, and the result lists reused/rendered chunks with the reason (no_previous, key_changed, file_missing, file_hash_mismatch, forced). force:true re-renders everything. wait:true blocks until the job ends and returns the final status. The job renders a snapshot of the project taken at start.",
        schema: obj(vec![("path", s("output .mp4 path")), ("chunk_s", n("chunk length in seconds, default 4")), ("crf", n("quality 0..51, default 18")), ("dir", s("chunk directory, default <path>.chunks")), ("force", json!({"type":"boolean"})), ("wait", json!({"type":"boolean","description":"block until done (default false)"}))], &["path"]),
        run: |ss, a| start(ss, a, false) },
    Command { name: "job_status", mutates: false,
        description: "Status of an export job: state (queued|running|cancelling|cancelled|failed|done), frames_done/frames_total, elapsed_s, current_range [first,last) frames, chunks {done,total,reused}, warnings, completed_ranges, and eta {seconds, uncertainty low|medium|high} which stays null until 3 chunks have been measured. The final result (path, reused/rendered chunks with reasons) is under result.",
        schema: obj(vec![("job_id", s("id returned by export_job_start"))], &["job_id"]), run: |_ss, a| { let j = get(a)?; Ok(status_json(&j)) } },
    Command { name: "cancel_job", mutates: false,
        description: "Cancel an export job. Waits up to 20 s for it to stop, then returns the status plus completed_ranges (chunks that finished and stay valid). An unfinished chunk is renamed chunk_NNNNN.mp4.partial with a .partial.marker file so it is never mistaken for a finished one.",
        schema: obj(vec![("job_id", s("job to cancel"))], &["job_id"]),
        run: |_ss, a| { let j = get(a)?; set(&j, |s| if s.state == "running" || s.state == "queued" { s.state = "cancelling"; }); j.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            for _ in 0..200 { { let s = j.st.lock().unwrap_or_else(|e| e.into_inner()); if !matches!(s.state, "cancelling" | "running" | "queued") { break; } } std::thread::sleep(std::time::Duration::from_millis(100)); } Ok(status_json(&j)) } },
    Command { name: "resume_job", mutates: false,
        description: "Continue an interrupted or cancelled export from its chunk directory. Verifies the project still matches the manifest written when the job started and refuses (error resume_mismatch) naming the first differing dependency (use export_job_start to reuse whatever is still valid instead). Finished chunks whose key and file hash verify are kept; the rest are rendered.",
        schema: obj(vec![("path", s("output .mp4 path")), ("dir", s("chunk directory, default <path>.chunks")), ("crf", n("quality 0..51, default 18")), ("chunk_s", n("default: the original job's"))], &["path"]),
        run: |ss, a| start(ss, a, true) },
] }
