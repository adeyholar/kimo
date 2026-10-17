//! Kimo spectator: read-only live view of engine tool execution.
//!
//! The engine (kimo-mcp or a script) streams newline-delimited JSON tool events
//! over loopback TCP; this shell binds the first free port of 127.0.0.1:7421..=7430
//! (loopback only, never 0.0.0.0), logs which one it took, greets every connection
//! with the versioned hello line kimo-mcp waits for (`HELLO`), and forwards the events to the
//! webview. kimo-mcp finds it there by default, with nothing to set.
//! READ-ONLY BY CONSTRUCTION: there are no IPC commands at all - the frontend
//! can only listen. Event schema (all fields except `type` optional):
//!   {"type":"tool_start","id":"c7","name":"color_grade",
//!    "inputs":[{"port":"in","from":"load_media:out"}],"args":{"gamma":1.04}}
//!   {"type":"tool_end","id":"c7","dur_ms":14,"status":"ok"}
//!   {"type":"project_changed","rev":214}
//! Zero runtime network: loopback only, every asset vendored.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use tauri::{Emitter, Manager};

/// The loopback ports kimo-mcp scans by default (crates/kimo-mcp/src/spectator.rs, `GUI_PORTS`).
const PORTS: std::ops::RangeInclusive<u16> = 7421..=7430;
/// The first line on every accepted connection; kimo-mcp keeps a connection only after reading it (`HELLO` there).
const HELLO: &[u8] = b"KIMO-SPECTATOR/1\n";

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || listen_loop(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("kimo spectator failed to start");
}

/// Accept loopback connections and forward each JSONL event to the webview.
/// Reconnects forever; a dropped engine never kills the spectator.
fn listen_loop(handle: tauri::AppHandle) {
    loop {
        let listener = match PORTS.clone().find_map(|p| TcpListener::bind(("127.0.0.1", p)).ok()) {
            Some(l) => l,
            None => {
                let _ = handle.emit("spectator-status", "bind failed: 127.0.0.1:7421-7430 all in use");
                std::thread::sleep(std::time::Duration::from_secs(2));
                continue;
            }
        };
        // the status pill (ui/bridge.js) shows exactly "listening 127.0.0.1:<port>" for the port this shell bound
        let status = listener.local_addr().map_or_else(|_| "listening".to_string(), |a| format!("listening {a}"));
        eprintln!("kimo spectator: {status}");
        if let Some(w) = handle.get_webview_window("main") {
            let _ = w.set_title(&format!("Kimo \u{b7} spectator \u{b7} {status}"));
        }
        let _ = handle.emit("spectator-status", status.as_str());
        for stream in listener.incoming() {
            match stream {
                Ok(mut s) => {
                    if s.write_all(HELLO).is_err() {
                        continue; // the engine went away before the hello
                    }
                    let _ = handle.emit("spectator-status", "live");
                    for line in BufReader::new(s).lines() {
                        let line = match line {
                            Ok(l) => l,
                            Err(_) => break, // engine closed; back to accept
                        };
                        if line.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Value>(&line) {
                            Ok(v) => {
                                let _ = handle.emit("engine-event", v);
                            }
                            Err(e) => {
                                let _ = handle.emit("spectator-status", format!("bad json: {e}"));
                            }
                        }
                    }
                    let _ = handle.emit("spectator-status", status.as_str());
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(200)),
            }
        }
    }
}
