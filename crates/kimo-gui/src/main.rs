//! Kimo spectator: read-only live view of engine tool execution.
//!
//! The engine (kimo-mcp or a script) streams newline-delimited JSON tool events
//! over loopback TCP 127.0.0.1:7421; this shell forwards them to the webview.
//! READ-ONLY BY CONSTRUCTION: there are no IPC commands at all - the frontend
//! can only listen. Event schema (all fields except `type` optional):
//!   {"type":"tool_start","id":"c7","name":"color_grade",
//!    "inputs":[{"port":"in","from":"load_media:out"}],"args":{"gamma":1.04}}
//!   {"type":"tool_end","id":"c7","dur_ms":14,"status":"ok"}
//!   {"type":"project_changed","rev":214}
//! Zero runtime network: loopback only, every asset vendored.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use tauri::Emitter;

const BIND: &str = "127.0.0.1:7421";

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
        let listener = match TcpListener::bind(BIND) {
            Ok(l) => l,
            Err(e) => {
                let _ = handle.emit("spectator-status", format!("bind failed: {e}"));
                std::thread::sleep(std::time::Duration::from_secs(2));
                continue;
            }
        };
        let _ = handle.emit("spectator-status", "listening");
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
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
                    let _ = handle.emit("spectator-status", "listening");
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(200)),
            }
        }
    }
}
