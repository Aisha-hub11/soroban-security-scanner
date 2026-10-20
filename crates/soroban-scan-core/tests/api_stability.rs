use std::path::PathBuf;

const STABLE: &[&str] = &[
    "category",
    "confidence",
    "config",
    "engine",
    "finding",
    "model",
    "registry",
    "report",
    "rule",
    "severity",
    "validation",
];

#[test]
fn every_public_module_is_classified_in_the_policy() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let policy = std::fs::read_to_string(root.join("../../docs/api-stability.md")).unwrap();

    for line in lib.lines() {
        let Some(rest) = line.trim().strip_prefix("pub mod ") else {
            continue;
        };
        let module = rest.trim_end_matches(';').trim();
        assert!(
            STABLE.contains(&module) || policy.contains(&format!("`{module}`")),
            "public module `{module}` is not classified as stable or unstable"
        );
    }
}
