//! Offline contributor-backlog rendering.
//!
//! Reads `tools/backlog/*.json` (the same files `tools/create_backlog.py`
//! consumes) and renders each issue body into a Markdown file. Creating the
//! issues on GitHub still requires `gh`; that stays in the Python tool until
//! xtask reaches parity.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// A rendered backlog issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Source JSON file name.
    pub source: String,
    /// Issue title.
    pub title: String,
    /// Rendered Markdown body.
    pub body: String,
}

const REPO_URL: &str = "https://github.com/AyinkxLab/soroban-security-scanner";
const DEFAULT_SECURITY: &str = "None beyond the project-wide guarantees.";
const DEFAULT_DEPENDENCIES: &str = "None.";
const DEFAULT_COMPLEXITY: &str = "medium";
const DEFAULT_DOD: &str = "Merged with tests passing and CI green.";

/// Loads and validates every backlog issue.
pub fn load(root: &Path) -> Result<Vec<Issue>, String> {
    let directory = root.join("tools").join("backlog");
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory)
        .map_err(|err| format!("failed to read {}: {err}", directory.display()))?
    {
        let entry =
            entry.map_err(|err| format!("failed to read {}: {err}", directory.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && entry.file_name().to_string_lossy() != "labels.json"
        {
            files.push(path);
        }
    }
    files.sort();

    if files.is_empty() {
        return Err(format!("no backlog files found in {}", directory.display()));
    }

    let mut issues = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|err| format!("invalid JSON in {}: {err}", path.display()))?;
        let entries = value
            .as_array()
            .ok_or_else(|| format!("{} must contain a JSON array", path.display()))?;
        let source = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for entry in entries {
            let title = text_field(entry, "title")?;
            issues.push(Issue {
                source: source.clone(),
                title: title.clone(),
                body: render_body(entry, &title)?,
            });
        }
    }

    let mut seen = BTreeSet::new();
    for issue in &issues {
        if !seen.insert(issue.title.clone()) {
            return Err(format!("duplicate issue title: {}", issue.title));
        }
    }

    Ok(issues)
}

/// Writes one Markdown file per issue into `output`.
pub fn write_all(root: &Path, output: &Path) -> Result<usize, String> {
    let issues = load(root)?;
    fs::create_dir_all(output)
        .map_err(|err| format!("failed to create {}: {err}", output.display()))?;

    for (index, issue) in issues.iter().enumerate() {
        let name = format!("{:03}-{}.md", index + 1, slug(&issue.title));
        let path = output.join(name);
        fs::write(&path, &issue.body)
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }
    Ok(issues.len())
}

fn render_body(issue: &Value, title: &str) -> Result<String, String> {
    let problem = text_field(issue, "problem")?;
    let why = text_field(issue, "why")?;

    let mut body = String::new();
    body.push_str("## Problem\n");
    body.push_str(&problem);
    body.push_str("\n\n## Why it matters\n");
    body.push_str(&why);
    body.push_str("\n\n## Scope\n");
    body.push_str(&bullet(issue.get("scope"))?);
    body.push_str("\n\n## Technical guidance\n");
    body.push_str(&bullet(issue.get("guidance"))?);
    body.push_str("\n\n## Acceptance criteria\n");
    body.push_str(&bullet(issue.get("acceptance"))?);
    body.push_str("\n\n## Tests\n");
    body.push_str(&bullet(issue.get("tests"))?);
    body.push_str("\n\n## Security considerations\n");
    body.push_str(&string_field(issue, "security", DEFAULT_SECURITY)?);
    body.push_str("\n\n## Dependencies\n");
    body.push_str(&string_field(issue, "deps", DEFAULT_DEPENDENCIES)?);
    body.push_str("\n\n## Complexity\n");
    body.push_str(&format!(
        "`difficulty/{}`",
        string_field(issue, "complexity", DEFAULT_COMPLEXITY)?
    ));
    body.push_str("\n\n## Definition of done\n");
    body.push_str(&string_field(issue, "dod", DEFAULT_DOD)?);
    body.push('\n');
    body.push_str(&render_guidelines(title));
    Ok(body)
}

fn render_guidelines(title: &str) -> String {
    [
        "## Contribution guidelines".to_string(),
        String::new(),
        "- **Assignment required before starting.** Comment on this issue to request assignment before writing code."
            .to_string(),
        "- Your pull request must include `Closes #` followed by this issue's number.".to_string(),
        "- Run the local gate before opening the PR:".to_string(),
        String::new(),
        "  ```".to_string(),
        "  cargo fmt --all -- --check".to_string(),
        "  cargo clippy --all-targets --all-features -- -D warnings".to_string(),
        "  cargo test --all".to_string(),
        "  ```".to_string(),
        String::new(),
        format!("- Example commit message: `{title}`"),
        "- Complexity is tagged with a `difficulty/*` label and maps to wave points (trivial 100 / medium 150 / high 200)."
            .to_string(),
        format!(
            "- See [docs/WAVE.md]({REPO_URL}/blob/main/docs/WAVE.md) and [CONTRIBUTING.md]({REPO_URL}/blob/main/CONTRIBUTING.md)."
        ),
        String::new(),
    ]
    .join("\n")
}

fn bullet(value: Option<&Value>) -> Result<String, String> {
    match value {
        None | Some(Value::Null) => Ok("_None._".to_string()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(Value::Array(items)) => {
            let mut lines = Vec::with_capacity(items.len());
            for item in items {
                let text = item
                    .as_str()
                    .ok_or_else(|| "bullet list entries must be strings".to_string())?;
                lines.push(format!("- {text}"));
            }
            if lines.is_empty() {
                Ok("_None._".to_string())
            } else {
                Ok(lines.join("\n"))
            }
        }
        Some(_) => Err("expected a string or an array of strings".to_string()),
    }
}

fn text_field(issue: &Value, field: &str) -> Result<String, String> {
    issue
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("issue is missing string field {field:?}"))
}

fn string_field(issue: &Value, field: &str, default: &str) -> Result<String, String> {
    match issue.get(field) {
        None | Some(Value::Null) => Ok(default.to_string()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(format!("field {field:?} must be a string")),
    }
}

fn slug(title: &str) -> String {
    let mut slug = String::new();
    let mut pending_separator = false;
    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            pending_separator = false;
            slug.push(character.to_ascii_lowercase());
        } else {
            pending_separator = true;
        }
    }
    if slug.is_empty() {
        "issue".to_string()
    } else {
        slug
    }
}

/// Default output directory, relative to the workspace root.
pub fn default_output(root: &Path) -> PathBuf {
    root.join("target").join("backlog")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn issue() -> Value {
        json!({
            "title": "feat(tooling): example",
            "problem": "Something is missing.",
            "why": "It matters.",
            "scope": ["first", "second"],
            "acceptance": "It works.",
            "complexity": "high",
            "labels": ["tooling"]
        })
    }

    #[test]
    fn renders_required_sections_and_defaults() {
        let body = render_body(&issue(), "feat(tooling): example").unwrap();
        assert!(body.starts_with("## Problem\nSomething is missing."));
        assert!(body.contains("## Scope\n- first\n- second"));
        assert!(body.contains("## Tests\n_None._"));
        assert!(body.contains("## Security considerations\nNone beyond"));
        assert!(body.contains("## Dependencies\nNone."));
        assert!(body.contains("`difficulty/high`"));
        assert!(body.contains("## Contribution guidelines"));
        assert!(body.contains("`feat(tooling): example`"));
    }

    #[test]
    fn rejects_non_string_bullets() {
        let mut value = issue();
        value["scope"] = json!([1, 2]);
        assert!(render_body(&value, "title").is_err());
    }

    #[test]
    fn slugs_are_stable_and_filesystem_safe() {
        assert_eq!(
            slug("feat(tooling): add an xtask crate"),
            "feat-tooling-add-an-xtask-crate"
        );
        assert_eq!(slug("!!!"), "issue");
    }
}
