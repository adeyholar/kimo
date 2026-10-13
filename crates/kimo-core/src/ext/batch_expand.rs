//! U2.2: `foreach` expansion for `batch`. A pure pre-pass: it turns {"foreach":{"var","in","max"},"ops":[...]} into the flat op list, substituting {{var}}, {{i}}, {{n}}.
//! Nothing runs during expansion except the read-only `find` source. No conditionals, no runtime loops.
use super::kit::*;
use crate::commands::Session;
use std::collections::HashSet;

pub const MAX_DEPTH: usize = 2;
pub const DEFAULT_MAX: usize = 500;
fn bad(m: String) -> String { format!("invalid_foreach: {m}") }
fn too_large(cap: usize) -> String { format!("batch_expansion_too_large: the expanded batch would exceed {cap} ops (the batch cap); lower the foreach range or split the batch") }
fn val_text(v: &Value) -> String { match v { Value::String(s) => s.clone(), o => o.to_string() } }
fn subst(v: &Value, env: &[(String, Value)]) -> Result<Value, String> {
    match v {
        Value::String(s) => {
            if let Some(name) = s.strip_prefix("{{").and_then(|r| r.strip_suffix("}}")).filter(|n| !n.contains("{{") && !n.contains("}}")) { let n = name.trim(); return env.iter().rev().find(|(k, _)| k == n).map(|(_, x)| x.clone()).ok_or(bad(format!("unknown variable '{{{{{n}}}}}'; defined here: {}", env.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", ")))); }
            if !s.contains("{{") { return Ok(v.clone()); }
            let mut out = String::new(); let mut rest = s.as_str();
            while let Some(a) = rest.find("{{") { out.push_str(&rest[..a]); let after = &rest[a + 2..]; let b = after.find("}}").ok_or(bad(format!("unterminated {{{{ in '{s}'")))?; let n = after[..b].trim();
                let x = env.iter().rev().find(|(k, _)| k == n).map(|(_, x)| x).ok_or(bad(format!("unknown variable '{{{{{n}}}}}'; defined here: {}", env.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", "))))?; out.push_str(&val_text(x)); rest = &after[b + 2..]; }
            out.push_str(rest); Ok(Value::String(out)) }
        Value::Array(a) => a.iter().map(|x| subst(x, env)).collect::<Result<Vec<_>, _>>().map(Value::Array),
        Value::Object(o) => { let mut m = serde_json::Map::new(); for (k, x) in o { m.insert(k.clone(), subst(x, env)?); } Ok(Value::Object(m)) }
        _ => Ok(v.clone()),
    }
}
/// "$id.x" -> "$id#sfx.x" for ids defined at this foreach level (a bare reference inside the body means the same iteration).
fn rewrite(v: &Value, defined: &HashSet<String>, sfx: &str) -> Value {
    match v {
        Value::String(s) if s.starts_with('$') && !s.starts_with("$$") => { let rest = &s[1..]; match rest.find(|c| c == '.' || c == '[') { Some(d) if defined.contains(&rest[..d]) => Value::String(format!("${}{}{}", &rest[..d], sfx, &rest[d..])), _ => v.clone() } }
        Value::Array(a) => Value::Array(a.iter().map(|x| rewrite(x, defined, sfx)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), rewrite(x, defined, sfx))).collect()),
        _ => v.clone(),
    }
}
fn defined_ids(body: &[Value]) -> HashSet<String> { body.iter().filter_map(|o| o.get("id").and_then(|v| v.as_str()).map(String::from)).collect() }
fn source(ss: &mut Session, src: &Value, env: &[(String, Value)], max: usize) -> Result<Vec<Value>, String> {
    let src = subst(src, env)?;
    match &src {
        Value::Array(a) => Ok(a.clone()),
        Value::Object(o) if o.len() == 1 && o.contains_key("range") => { let r = o["range"].as_array().filter(|r| r.len() == 2 || r.len() == 3).ok_or(bad("range must be [a,b] or [a,b,step]".into()))?;
            let f = |i: usize| r.get(i).and_then(|v| v.as_f64()).filter(|x| x.is_finite()); let (a, b) = (f(0).ok_or(bad("range bounds must be numbers".into()))?, f(1).ok_or(bad("range bounds must be numbers".into()))?); let st = if r.len() == 3 { f(2).ok_or(bad("range step must be a number".into()))? } else { 1.0 };
            if st == 0.0 { return Err(bad("range step must not be 0".into())); }
            let n = ((b - a) / st).ceil(); let n = if n.is_finite() && n > 0.0 { n as usize } else { 0 }; if n > max { return Err(bad(format!("range yields {n} items, max is {max}"))); }
            Ok((0..n).map(|k| { let x = a + st * k as f64; if x.fract() == 0.0 && x.abs() < 1e15 { json!(x as i64) } else { json!(x) } }).collect()) }
        Value::Object(o) if o.len() == 1 && o.contains_key("find") => { let r = crate::commands::dispatch_nested(ss, "find", &o["find"])?; Ok(r["matches"].as_array().map(|m| m.iter().filter_map(|x| x.get("id").cloned()).collect()).unwrap_or_default()) }
        _ => Err(bad("in must be an array, {\"range\":[a,b,step?]} or {\"find\":{...}}".into())),
    }
}
fn expand(ss: &mut Session, ops: &[Value], env: &[(String, Value)], depth: usize, prefix: &str, cap: usize, out: &mut Vec<Value>) -> Result<(), String> {
    for o in ops {
        if let Some(fe) = o.get("foreach") {
            if depth >= MAX_DEPTH { return Err(bad(format!("foreach nests at most {MAX_DEPTH} deep"))); }
            let var = fe.get("var").and_then(|v| v.as_str()).filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).ok_or(bad("foreach.var must be a name of letters, digits and _".into()))?;
            if var == "i" || var == "n" { return Err(bad("foreach.var must not be i or n (they are the index and count)".into())); }
            let max = match fe.get("max") { None | Some(Value::Null) => DEFAULT_MAX, Some(v) => v.as_u64().filter(|m| *m >= 1).ok_or(bad("foreach.max must be a whole number >= 1".into()))? as usize };
            let body = o.get("ops").and_then(|v| v.as_array()).filter(|b| !b.is_empty()).ok_or(bad("a foreach needs a non-empty ops array".into()))?;
            let items = source(ss, fe.get("in").ok_or(bad("foreach.in is required".into()))?, env, max)?; if items.len() > max { return Err(bad(format!("source has {} items, max is {max}", items.len()))); }
            let defined = defined_ids(body); let n = items.len();
            for (i, it) in items.into_iter().enumerate() {
                let sfx = format!("{prefix}#{i}"); let mut e2 = env.to_vec(); e2.push((var.to_string(), it)); e2.push(("i".into(), json!(i))); e2.push(("n".into(), json!(n)));
                let rewritten: Vec<Value> = body.iter().map(|b| { let mut b = rewrite(b, &defined, &sfx); if let Some(id) = b.get("id").and_then(|v| v.as_str()).filter(|_| b.get("foreach").is_none()) { let nid = format!("{id}{sfx}"); b["id"] = json!(nid); } b }).collect();
                expand(ss, &rewritten, &e2, depth + 1, &sfx, cap, out)?;
            }
        } else {
            if out.len() >= cap { return Err(too_large(cap)); }
            let mut leaf = o.clone(); if !env.is_empty() { if let Some(args) = o.get("args") { leaf["args"] = subst(args, env)?; } } out.push(leaf);
        }
        if out.len() > cap { return Err(too_large(cap)); }
    }
    Ok(())
}
/// Returns the batch args with `ops` flattened, or None when no op uses foreach (the common path is untouched).
pub fn expand_args(ss: &mut Session, a: &Value) -> Result<Option<Value>, String> {
    let Some(arr) = a.get("ops").and_then(|v| v.as_array()) else { return Ok(None) };
    if !arr.iter().any(|o| o.get("foreach").is_some()) { return Ok(None); }
    let mut out = vec![]; expand(ss, arr, &[], 0, "", super::batch::MAX_OPS, &mut out)?;
    let mut b = a.clone(); b["ops"] = Value::Array(out); Ok(Some(b))
}
pub fn commands() -> Vec<Command> { vec![] }
