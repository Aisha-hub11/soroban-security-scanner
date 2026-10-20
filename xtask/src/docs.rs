//! Offline relative-link checker for Markdown files.
//!
//! This is a Rust reimplementation of `tools/check_docs.py`. It never fetches
//! external URLs and never writes to the repository.

use std::fs;
use std::path::{Path, PathBuf};

const SKIP_DIRS: [&str; 4] = [".git", "target", "node_modules", "vendor"];

/// Checks every Markdown file under `root`.
///
/// Returns the number of files checked on success. On failure, returns a
/// human-readable report listing every broken relative link.
pub fn check(root: &Path) -> Result<usize, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }

    let mut files = Vec::new();
    collect_markdown(root, &mut files)
        .map_err(|err| format!("failed to walk {}: {err}", root.display()))?;
    files.sort();

    let mut problems = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file)
            .map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        for target in link_targets(&text) {
            if let Some(problem) = check_target(root, file, &target) {
                problems.push(problem);
            }
        }
    }

    if problems.is_empty() {
        Ok(files.len())
    } else {
        let mut report = format!(
            "Found {} broken link(s) across {} files:",
            problems.len(),
            files.len()
        );
        for problem in problems {
            report.push_str("\n  ");
            report.push_str(&problem);
        }
        Err(report)
    }
}

fn collect_markdown(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !SKIP_DIRS.iter().any(|skip| *skip == name) {
                collect_markdown(&path, files)?;
            }
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    Ok(())
}

/// Extracts inline Markdown link targets (`[text](target)` and images).
///
/// Reference-style links are intentionally ignored, matching the Python tool.
pub(crate) fn link_targets(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut targets = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'[' {
            index += 1;
            continue;
        }

        let Some(close) = matching(bytes, index, b'[', b']') else {
            break;
        };
        if bytes.get(close + 1) != Some(&b'(') {
            index = close + 1;
            continue;
        }
        let Some(end) = matching(bytes, close + 1, b'(', b')') else {
            break;
        };
        targets.push(text[close + 2..end].trim().to_string());
        index = end + 1;
    }

    targets
}

/// Finds the index of the delimiter matching `open` at `start`.
fn matching(bytes: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(start + 1) {
        if *byte == open {
            depth += 1;
        } else if *byte == close {
            if depth == 0 {
                return Some(offset);
            }
            depth -= 1;
        }
    }
    None
}

fn is_external(target: &str) -> bool {
    ["http://", "https://", "mailto:", "#", "<"]
        .iter()
        .any(|prefix| target.starts_with(prefix))
}

fn check_target(root: &Path, file: &Path, target: &str) -> Option<String> {
    if target.is_empty() || is_external(target) {
        return None;
    }
    let path = target.split('#').next().unwrap_or("").trim();
    if path.is_empty() {
        return None;
    }

    let resolved = if let Some(stripped) = path.strip_prefix('/') {
        root.join(stripped)
    } else {
        file.parent().unwrap_or(root).join(path)
    };
    if resolved.exists() {
        return None;
    }

    let relative = file.strip_prefix(root).unwrap_or(file);
    Some(format!("{}: broken link -> {target}", relative.display()))
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
        let root = std::env::temp_dir().join(format!("xtask-docs-{label}-{nanos}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn finds_inline_links_and_ignores_reference_links() {
        let targets = link_targets("[a](one.md) ![b](two.png) [c][ref] [d](#anchor)");
        assert_eq!(targets, vec!["one.md", "two.png", "#anchor"]);
    }

    #[test]
    fn reports_broken_relative_links_only() {
        let root = temp_root("broken");
        fs::write(root.join("good.md"), "[other](other.md)").unwrap();
        fs::write(root.join("other.md"), "# Other\n").unwrap();
        fs::write(
            root.join("broken.md"),
            "[missing](missing.md)\n[web](https://example.com)\n[anchor](#top)\n",
        )
        .unwrap();

        let error = check(&root).unwrap_err();
        assert!(
            error.contains("broken.md: broken link -> missing.md"),
            "{error}"
        );
        assert!(!error.contains("https://example.com"), "{error}");

        fs::remove_file(root.join("broken.md")).unwrap();
        assert_eq!(check(&root).unwrap(), 2);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn resolves_root_relative_links() {
        let root = temp_root("root-relative");
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/page.md"), "[readme](/README.md)").unwrap();
        fs::write(root.join("README.md"), "# Root\n").unwrap();

        assert_eq!(check(&root).unwrap(), 2);
        fs::remove_dir_all(&root).unwrap();
    }
}
