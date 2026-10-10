//! kino-mcp: Model Context Protocol server over stdio (newline-delimited JSON-RPC 2.0).
//! Tools are generated from kino_core::commands::registry().
use kino_core::commands::{dispatch, registry, Session};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn main() {
    let mut ss = Session::new();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() { continue; }
        let Ok(req): Result<Value, _> = serde_json::from_str(&line) else { continue };
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));
        let result: Option<Result<Value, (i64, String)>> = match method {
            "initialize" => Some(Ok(json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"kino","version":env!("CARGO_PKG_VERSION")}}))),
            "ping" => Some(Ok(json!({}))),
            "tools/list" => Some(Ok(json!({"tools": registry().iter().map(|c| json!({"name":c.name,"description":c.description,"inputSchema":c.schema})).collect::<Vec<_>>()}))),
            "tools/call" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Some(Ok(match dispatch(&mut ss, name, &args) {
                    Ok(v) => json!({"content":[{"type":"text","text":serde_json::to_string(&v).unwrap()}],"isError":false}),
                    Err(e) => json!({"content":[{"type":"text","text":e}],"isError":true}),
                }))
            }
            m if m.starts_with("notifications/") => None,
            _ => Some(Err((-32601, format!("method not found: {method}")))),
        };
        if let (Some(id), Some(r)) = (id, result) {
            let msg = match r { Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}), Err((c, m)) => json!({"jsonrpc":"2.0","id":id,"error":{"code":c,"message":m}}) };
            writeln!(out, "{msg}").ok(); out.flush().ok();
        }
    }
}
