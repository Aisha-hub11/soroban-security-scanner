//! Keeps the workspace layout document in sync with the real module tree (#72).

use std::path::PathBuf;

fn read(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

#[test]
fn workspace_layout_documents_every_public_module() {
    let lib = read("src/lib.rs");
    let layout = read("../../docs/architecture/workspace-layout.md");

    let modules: Vec<&str> = lib
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub mod ")
                .and_then(|rest| rest.strip_suffix(';'))
        })
        .collect();

    assert!(
        !modules.is_empty(),
        "expected at least one pub mod in src/lib.rs"
    );
    for module in modules {
        let marker = format!("`{module}`");
        assert!(
            layout.contains(&marker),
            "workspace-layout.md is missing public module {marker}; \
             add a row to the top-level module table"
        );
    }
}

#[test]
fn workspace_layout_lists_both_crates_and_dependency_rules() {
    let layout = read("../../docs/architecture/workspace-layout.md");
    for expected in [
        "soroban-scan-core",
        "soroban-scan-cli",
        "The core never depends on the CLI",
        "Public API",
        "Dependency rules",
    ] {
        assert!(
            layout.contains(expected),
            "workspace-layout.md is missing {expected:?}"
        );
    }
}
