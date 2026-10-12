//! The v3 README line quotes the tool count; it must equal the registry (keeps docs from drifting).
#[test] fn readme_v3_tool_count_matches_the_registry() {
    let r = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md")).unwrap();
    let line = r.lines().find(|l| l.starts_with("**v3 branch")).expect("README must have the **v3 branch line"); let n: usize = line.split(" tools").next().unwrap().rsplit(' ').next().unwrap().parse().expect("tool count");
    assert_eq!(n, kimo_core::commands::registry_ref().len(), "README says {n} tools");
}
