//! Scan and rule configuration.
//!
//! Repository-provided configuration is treated as untrusted data. No option
//! here can enable code execution or network access.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::ai::AiConfig;
use crate::category::Category;
use crate::confidence::Confidence;
use crate::severity::Severity;

/// Deserializes category names, accepting both kebab-case and snake_case.
///
/// Categories are validated here so an unknown name is a configuration error
/// rather than a silently ignored filter.
fn deserialize_categories<'de, D>(deserializer: D) -> Result<Vec<Category>, D::Error>
where
    D: Deserializer<'de>,
{
    let names = Vec::<String>::deserialize(deserializer)?;
    names
        .into_iter()
        .map(|name| name.parse::<Category>().map_err(serde::de::Error::custom))
        .collect()
}

/// Rule selection and overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    /// Rule ids to disable.
    pub disabled: Vec<String>,
    /// If non-empty, only these rule ids are enabled.
    pub enabled: Vec<String>,
    /// Categories to disable.
    #[serde(deserialize_with = "deserialize_categories")]
    pub disabled_categories: Vec<Category>,
    /// If non-empty, only rules in these categories are enabled.
    #[serde(deserialize_with = "deserialize_categories")]
    pub enabled_categories: Vec<Category>,
    /// Per-rule severity overrides.
    pub severity_overrides: BTreeMap<String, Severity>,
    /// Per-rule confidence overrides.
    pub confidence_overrides: BTreeMap<String, Confidence>,
    /// Optional per-rule wall-clock budget in milliseconds.
    ///
    /// Disabled by default. When set, each rule runs on a worker thread and is
    /// abandoned if it exceeds the budget; the engine emits a diagnostic and
    /// continues with the remaining rules. The budget is best-effort: a rule
    /// that has already started may keep running in the background until it
    /// returns, but its findings are discarded.
    #[serde(rename = "time_budget_ms", alias = "rule_time_budget_ms")]
    pub rule_time_budget_ms: Option<u64>,
}

impl RulesConfig {
    /// Returns true if the rule id is enabled under this configuration.
    pub fn is_enabled(&self, id: &str) -> bool {
        if self.disabled.iter().any(|r| r == id) {
            return false;
        }
        if !self.enabled.is_empty() {
            return self.enabled.iter().any(|r| r == id);
        }
        true
    }

    /// Returns true if the category is enabled under this configuration.
    ///
    /// Disabled categories always win. When `enabled_categories` is non-empty,
    /// a rule must also belong to one of the listed categories.
    pub fn is_category_enabled(&self, category: Category) -> bool {
        if self.disabled_categories.contains(&category) {
            return false;
        }
        if !self.enabled_categories.is_empty() {
            return self.enabled_categories.contains(&category);
        }
        true
    }

    /// Returns true if a rule is enabled by id and category.
    pub fn is_rule_enabled(&self, id: &str, category: Category) -> bool {
        self.is_enabled(id) && self.is_category_enabled(category)
    }
}

/// Complete scan configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScanConfig {
    /// Minimum severity to report.
    pub min_severity: Severity,
    /// Minimum confidence to report.
    pub min_confidence: Confidence,
    /// Glob patterns (relative to the scan root) to exclude from discovery.
    pub exclude: Vec<String>,
    /// Skip items gated behind `#[cfg(test)]` and test directories.
    pub exclude_tests: bool,
    /// Whether to follow symbolic links during discovery.
    pub follow_symlinks: bool,
    /// Minimum supported `soroban-sdk` version for the SS-015 policy check.
    pub minimum_soroban_sdk: String,
    /// Rule selection and overrides.
    pub rules: RulesConfig,
    /// Optional assistance layer configuration (disabled by default).
    pub ai: AiConfig,
}

impl Default for ScanConfig {
    fn default() -> Self {
        ScanConfig {
            min_severity: Severity::Info,
            min_confidence: Confidence::Low,
            exclude: Vec::new(),
            exclude_tests: false,
            follow_symlinks: false,
            minimum_soroban_sdk: "21.0.0".to_string(),
            rules: RulesConfig::default(),
            ai: AiConfig::default(),
        }
    }
}

impl ScanConfig {
    /// Builds [`crate::discovery::DiscoveryOptions`] from this configuration.
    pub fn discovery_options(&self) -> crate::discovery::DiscoveryOptions {
        crate::discovery::DiscoveryOptions {
            follow_symlinks: self.follow_symlinks,
            exclude: self.exclude.clone(),
            exclude_tests: self.exclude_tests,
        }
    }

    /// Parses a configuration from TOML text.
    ///
    /// Deserialization rejects unknown fields; the result is then validated so
    /// out-of-range assistance parameters fail as configuration errors.
    pub fn from_toml(text: &str) -> Result<Self, crate::error::ScanError> {
        let config: Self = text
            .parse::<toml::Value>()
            .map_err(|e| crate::error::ScanError::Config(format!("invalid config TOML: {e}")))
            .and_then(|value| {
                value
                    .try_into()
                    .map_err(|e| crate::error::ScanError::Config(format!("invalid config: {e}")))
            })?;
        config
            .ai
            .validate()
            .map_err(|e| crate::error::ScanError::Config(format!("invalid config: {e}")))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_permissive_and_safe() {
        let config = ScanConfig::default();
        assert_eq!(config.min_severity, Severity::Info);
        assert_eq!(config.min_confidence, Confidence::Low);
        assert!(!config.follow_symlinks);
    }

    #[test]
    fn rule_enable_disable_logic() {
        let rules = RulesConfig::default();
        assert!(rules.is_enabled("SS-001"));

        let rules = RulesConfig {
            disabled: vec!["SS-001".into()],
            ..RulesConfig::default()
        };
        assert!(!rules.is_enabled("SS-001"));

        let only = RulesConfig {
            enabled: vec!["SS-002".into()],
            ..RulesConfig::default()
        };
        assert!(only.is_enabled("SS-002"));
        assert!(!only.is_enabled("SS-001"));
    }

    #[test]
    fn parses_toml_config() {
        let config = ScanConfig::from_toml(
            r#"
min_severity = "medium"
min_confidence = "low"
exclude = ["vendor/**"]

[rules]
disabled = ["SS-010"]
"#,
        )
        .unwrap();
        assert_eq!(config.min_severity, Severity::Medium);
        assert!(!config.rules.is_enabled("SS-010"));
    }

    #[test]
    fn rule_time_budget_is_disabled_by_default_and_parses() {
        assert_eq!(RulesConfig::default().rule_time_budget_ms, None);

        let config = ScanConfig::from_toml(
            r#"
[rules]
time_budget_ms = 2500
"#,
        )
        .unwrap();
        assert_eq!(config.rules.rule_time_budget_ms, Some(2500));
    }

    #[test]
    fn parses_and_applies_category_filters() {
        let config = ScanConfig::from_toml(
            r#"
[rules]
disabled_categories = ["resource-usage"]
enabled_categories = ["authorization", "storage"]
"#,
        )
        .unwrap();

        assert_eq!(
            config.rules.disabled_categories,
            vec![Category::ResourceUsage]
        );
        assert_eq!(
            config.rules.enabled_categories,
            vec![Category::Authorization, Category::Storage]
        );
        assert!(!config.rules.is_category_enabled(Category::ResourceUsage));
        assert!(config.rules.is_category_enabled(Category::Authorization));
        assert!(config.rules.is_category_enabled(Category::Storage));
        assert!(!config.rules.is_category_enabled(Category::Arithmetic));
    }

    #[test]
    fn accepts_snake_case_category_names() {
        let config = ScanConfig::from_toml(
            r#"
[rules]
disabled_categories = ["resource_usage", "access_control"]
"#,
        )
        .unwrap();
        assert_eq!(
            config.rules.disabled_categories,
            vec![Category::ResourceUsage, Category::AccessControl]
        );
    }

    #[test]
    fn rejects_unknown_category() {
        let err = ScanConfig::from_toml(
            r#"
[rules]
disabled_categories = ["nonsense"]
"#,
        )
        .unwrap_err();
        assert!(matches!(err, crate::error::ScanError::Config(_)));
        assert!(
            err.to_string().contains("unknown category"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = ScanConfig::from_toml("mystery = true").unwrap_err();
        assert!(matches!(err, crate::error::ScanError::Config(_)));
    }

    #[test]
    fn parses_validated_ai_parameters() {
        let config = ScanConfig::from_toml(
            r#"
[ai]
enabled = false
provider = "openai"
model = "example-model"
temperature = 0.2
max_output_tokens = 2048
"#,
        )
        .unwrap();
        assert_eq!(config.ai.temperature, 0.2);
        assert_eq!(config.ai.max_output_tokens, 2048);
        assert_eq!(config.ai.provider.as_deref(), Some("openai"));
    }

    #[test]
    fn defaults_include_conservative_ai_parameters() {
        let config = ScanConfig::default();
        assert_eq!(config.ai.temperature, 0.0);
        assert_eq!(config.ai.max_output_tokens, 1024);
    }

    #[test]
    fn rejects_out_of_range_ai_parameters() {
        let cases = [
            ("[ai]\ntemperature = 1.5\n", "ai.temperature"),
            ("[ai]\ntemperature = -0.1\n", "ai.temperature"),
            ("[ai]\ntemperature = nan\n", "ai.temperature"),
            ("[ai]\nmax_output_tokens = 0\n", "ai.max_output_tokens"),
            ("[ai]\nmax_output_tokens = 9000\n", "ai.max_output_tokens"),
        ];
        for (text, field) in cases {
            let err = ScanConfig::from_toml(text).unwrap_err();
            assert!(matches!(err, crate::error::ScanError::Config(_)));
            assert!(
                err.to_string().contains(field),
                "expected {field} in error: {err}"
            );
        }
    }
}
