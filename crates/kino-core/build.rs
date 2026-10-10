// Auto-discovers extension modules: every src/ext/<name>.rs (and src/ext/<name>/mod.rs) becomes `ext::<name>`,
// and its `pub fn commands() -> Vec<Command>` is added to the registry. New tools never need to edit an existing file.
use std::{env, fs, path::PathBuf};
fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src/ext"); println!("cargo:rerun-if-changed=src/ext");
    let mut names: Vec<(String, PathBuf)> = vec![];
    if let Ok(rd) = fs::read_dir(&dir) { for e in rd.flatten() { let p = e.path(); let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if stem == "mod" || stem.starts_with('.') || stem.is_empty() { continue; }
        if p.is_dir() { if p.join("mod.rs").exists() { names.push((stem, p.join("mod.rs"))); } } else if p.extension().and_then(|s| s.to_str()) == Some("rs") { names.push((stem, p)); } } }
    names.sort(); let mut out = String::new();
    for (n, p) in &names { out += &format!("#[path = {:?}] pub mod {};\n", p.to_string_lossy(), n); }
    out += "pub fn commands() -> Vec<crate::commands::Command> { let mut v = Vec::new();\n"; for (n, _) in &names { out += &format!("    v.extend({}::commands());\n", n); } out += "    v }\n";
    out += &format!("pub const MODULES: &[&str] = &[{}];\n", names.iter().map(|(n, _)| format!("{:?}", n)).collect::<Vec<_>>().join(", "));
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("ext_gen.rs"), out).unwrap();
}
