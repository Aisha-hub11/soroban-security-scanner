# Configuration

The scanner reads configuration from `soroban-scan.toml` in the scan root or the
current directory, unless `--config` selects a specific file. Command-line
options override the file.

`--no-config` skips discovery entirely and runs with the built-in defaults. Use it
when debugging, or when scanning an untrusted repository whose own
`soroban-scan.toml` must not influence the scan. It is mutually exclusive
with `--config`.

Generate a starter file:

```bash
soroban-scan init
```

Use `--interactive` to answer a few questions about minimum severity, enabled
categories, and exclude globs; the wizard then writes a valid configuration
file. Piped input is supported for scripted setup.

```bash
soroban-scan init --interactive
```

Inspect the effective configuration and validate it:

```bash
soroban-scan config show
soroban-scan config validate
```

## Reference

```toml
# Minimum severity to report: info, low, medium, high, critical
min_severity = "info"

# Minimum confidence to report: low, medium, high
min_confidence = "low"

# Glob patterns (relative to the scan root) to exclude from discovery
exclude = ["vendor/**", "target/**"]

# Skip #[cfg(test)] items and test directories
exclude_tests = false

# Follow symbolic links during discovery (disabled by default for safety)
follow_symlinks = false

[rules]
# Rule ids to disable
disabled = ["SS-006"]

# If non-empty, only these rule ids run
enabled = []

# Categories to disable. Category names are validated when the file is loaded;
# an unknown category is a usage error. Accepts kebab-case or snake_case.
disabled_categories = ["resource-usage"]

# If non-empty, only rules in these categories run
enabled_categories = []

# Per-rule severity overrides
severity_overrides = { "SS-007" = "low" }

# Per-rule confidence overrides
confidence_overrides = { }

[ai]
# Optional assistance layer. Disabled by default; the deterministic scanner is
# authoritative. See docs/ai.md.
enabled = false
# provider = "openai"
# model = "..."
```

### Fields

| Field | Type | Default | Meaning |
| ----- | ---- | ------- | ------- |
| `min_severity` | severity | `info` | Minimum severity to report. |
| `min_confidence` | confidence | `low` | Minimum confidence to report. |
| `exclude` | array of globs | `[]` | Paths excluded from discovery. |
| `exclude_tests` | bool | `false` | Skip `#[cfg(test)]` items and directories named `tests`. |
| `follow_symlinks` | bool | `false` | Follow symlinks (off by default for safety). |
| `rules.disabled` | array of ids | `[]` | Rules never run. |
| `rules.enabled` | array of ids | `[]` | If non-empty, the only rules that run. |
| `rules.disabled_categories` | array of categories | `[]` | Rules in these categories never run. |
| `rules.enabled_categories` | array of categories | `[]` | If non-empty, only rules in these categories run. |
| `rules.severity_overrides` | map id→severity | `{}` | Replace a rule's default severity. |
| `rules.confidence_overrides` | map id→confidence | `{}` | Replace a rule's default confidence. |
| `ai.enabled` | bool | `false` | Enable the optional assistance layer (fails closed without a provider). |
| `ai.provider` | string | *(none)* | Provider name; none is bundled. |
| `ai.model` | string | *(none)* | Model identifier. |

Unknown fields are rejected, as are references to unknown rule ids or
categories.

Category filters are applied before per-rule severity and confidence overrides.
A rule is skipped when it is individually disabled, when it belongs to a
disabled category, or when `enabled_categories` is non-empty and does not list
its category. The valid categories are:

`authorization`, `authentication`, `access-control`, `cross-contract`, `token`,
`storage`, `arithmetic`, `error-handling`, `events`, `resource-usage`,
`configuration`, `dependencies`, `unsafe`.

For example, this disables all resource-usage heuristics while keeping the rest
of the default rule set:

```toml
[rules]
disabled_categories = ["resource-usage"]
```

JSON reports include this effective configuration (after precedence below) in
the `effective_config` object, so a report can be reproduced from its own
contents. The object contains no environment data, credentials, or absolute
user paths.

## Precedence

1. Built-in defaults.
2. Configuration file (`--config`, scan root, or current directory); skipped
   entirely with `--no-config`.
3. Command-line flags (`--min-severity`, `--min-confidence`, `--exclude`,
   `--disable`, `--rule`).

## Example: strict CI policy

```toml
min_severity = "medium"
min_confidence = "medium"
exclude = ["tests/**", "examples/**"]

[rules]
disabled = ["SS-006"]
severity_overrides = { "SS-002" = "high" }
```

Combined with `--fail-on high`, medium findings are reported but only high and
critical findings fail the build.

## Security

Configuration files are untrusted data. No option can enable code execution,
network access, or access to secrets. See
[Security Model](security/security-model.md).
