//! Project fixture scaffolding.
//!
//! Fixtures are generated under `fixtures/projects/<name>` and are intentionally
//! tiny: they exist to be parsed, not compiled.

use std::fs;
use std::path::{Path, PathBuf};

/// Scaffolds a new project fixture and returns its directory.
///
/// Existing directories are refused unless `force` is set, in which case only
/// the scaffold files are overwritten. Unrelated files are never deleted.
pub fn scaffold(root: &Path, name: &str, force: bool) -> Result<PathBuf, String> {
    validate_name(name)?;

    let directory = root.join("fixtures").join("projects").join(name);
    if directory.exists() && !force {
        return Err(format!(
            "fixture already exists: {} (pass --force to overwrite its scaffold files)",
            directory.display()
        ));
    }

    let crate_name = format!("fixture-{}", name.replace('_', "-"));
    fs::create_dir_all(directory.join("src"))
        .map_err(|err| format!("failed to create {}: {err}", directory.display()))?;

    let manifest = format!(
        "[package]\n\
         name = \"{crate_name}\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         publish = false\n\
         \n\
         [lib]\n\
         path = \"src/lib.rs\"\n\
         \n\
         [dependencies]\n"
    );
    let source = format!(
        "//! Fixture `{name}`, scaffolded by `cargo xtask fixtures`.\n\
         //!\n\
         //! Replace this body with the smallest reproduction of the behaviour\n\
         //! under test. Fixtures are parsed by the scanner, not compiled.\n\
         \n\
         pub fn add(a: u64, b: u64) -> u64 {{\n\
         \x20   a + b\n\
         }}\n"
    );

    write(&directory.join("Cargo.toml"), &manifest, force)?;
    write(&directory.join("src/lib.rs"), &source, force)?;
    Ok(directory)
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("fixture name must not be empty".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "invalid fixture name {name:?}: use only letters, digits, '-' and '_'"
        ));
    }
    if name.starts_with('-') || name.starts_with('_') {
        return Err(format!(
            "invalid fixture name {name:?}: must start with a letter or digit"
        ));
    }
    Ok(())
}

fn write(path: &Path, contents: &str, force: bool) -> Result<(), String> {
    if path.exists() && !force {
        return Err(format!(
            "refusing to overwrite {} (pass --force)",
            path.display()
        ));
    }
    fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("xtask-fixtures-{label}-{nanos}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn scaffolds_a_parseable_fixture() {
        let root = temp_root("scaffold");
        let directory = scaffold(&root, "nested-call", false).unwrap();
        assert!(directory.join("Cargo.toml").is_file());
        let source = fs::read_to_string(directory.join("src/lib.rs")).unwrap();
        assert!(source.contains("pub fn add"));
        assert!(source.contains("nested-call"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_without_force() {
        let root = temp_root("overwrite");
        scaffold(&root, "sample", false).unwrap();
        let error = scaffold(&root, "sample", false).unwrap_err();
        assert!(error.contains("already exists"), "{error}");
        assert!(scaffold(&root, "sample", true).is_ok());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_path_traversal() {
        let root = temp_root("traversal");
        for name in ["../escape", "a/b", "a\\b", "", ".hidden"] {
            assert!(scaffold(&root, name, false).is_err(), "accepted {name:?}");
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
