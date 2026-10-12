//! Structured tool errors (unit 5, step 1). `dispatch` still returns `Result<Value, String>`; `dispatch_ex` wraps the same text in a `ToolError` with a
//! machine-readable code, the offending parameter and a hint. The text (`message`) is byte-identical to the string error, so old clients are unchanged.
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ToolError { pub code: &'static str, pub message: String, #[serde(skip_serializing_if = "Option::is_none")] pub param: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] pub hint: Option<String> }

fn quoted(s: &str) -> Option<String> { let a = s.find('\'')?; let b = s[a + 1..].find('\'')?; Some(s[a + 1..a + 1 + b].to_string()) }

/// Classify an error text by prefix/pattern. Returns (code, param).
pub fn classify(m: &str) -> (&'static str, Option<String>) {
    if m.contains("has no time; use set_keyframe") { return ("invalid_argument_combo", Some("time".to_string())); }
    if m.starts_with("unknown argument") { return ("unknown_argument", quoted(m)); }
    if m.starts_with("unknown tool") { return ("unknown_tool", quoted(m).map(|_| "tool".to_string())); }
    if m.starts_with("rejected by sandbox") { return ("sandbox_rejected", None); }
    if m.contains("failed safely") { return ("internal_panic", None); }
    if m.contains("stopped: exceeded") || m.contains("hung past") { return ("timeout", None); }
    if let Some(r) = m.strip_prefix("argument ") { if r.contains("' must be a") || r.contains("' must be an") { return ("bad_arg_type", quoted(&m["argument ".len() - 0..])); } }
    for ty in ["string", "number", "boolean", "array", "object", "integer"] { if let Some(r) = m.strip_prefix(&format!("missing {ty} ")) { return ("missing_arg", quoted(r)); } }
    if m.starts_with("cannot read '") || m.contains("(os error") { return ("io_error", None); }
    if m.contains("is locked") { return ("track_locked", None); }
    if m.contains("outside") || m.contains("strictly inside") || m.contains("must be between") || (m.contains("must be") && (m.contains("..") || m.contains(">=") || m.contains("> 0") || m.contains("<="))) { return ("out_of_range", None); }
    if m.starts_with("no such ") || m.starts_with("no clip ") || m.starts_with("missing ") || m.starts_with("no track ") { return ("not_found", None); }
    if m.contains("overlap") || m.contains("same track") || m.contains("sits within") || m.contains("must differ") || m.contains("already") { return ("invalid_state", None); }
    if m.starts_with("unknown ") { return ("invalid_value", None); }
    ("other", None)
}
impl From<String> for ToolError { fn from(m: String) -> Self { let (code, param) = classify(&m); ToolError { code, message: m, param, hint: None } } }
impl From<&str> for ToolError { fn from(m: &str) -> Self { m.to_string().into() } }

fn lev(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect()); let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() { let mut cur = vec![i]; for j in 1..=b.len() { cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + (a[i - 1] != b[j - 1]) as usize)); } prev = cur; } prev[b.len()]
}
/// Nearest `n` names by edit distance.
pub fn nearest(name: &str, names: &[String], n: usize) -> Vec<String> { let mut v: Vec<(usize, &String)> = names.iter().map(|x| (lev(name, x), x)).collect(); v.sort(); v.into_iter().take(n).map(|x| x.1.clone()).collect() }

/// Canonical text for a missing or wrong-typed argument: absent/null -> "missing <ty> 'k'", present but wrong type -> "argument 'k' must be a <ty> (got <actual>)".
pub fn arg_err(a: &serde_json::Value, k: &str, ty: &str) -> String {
    match a.get(k) { None | Some(serde_json::Value::Null) => format!("missing {ty} '{k}'"), Some(v) => { let got = match v { serde_json::Value::String(_) => "string", serde_json::Value::Number(_) => "number", serde_json::Value::Bool(_) => "boolean", serde_json::Value::Array(_) => "array", serde_json::Value::Object(_) => "object", serde_json::Value::Null => "null" }; format!("argument '{k}' must be a {ty} (got {got})") } }
}

pub fn lev_pub(a: &str, b: &str) -> usize { lev(a, b) }
