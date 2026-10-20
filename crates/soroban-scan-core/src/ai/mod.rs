//! Optional assistance layer.
//!
//! Design constraints (non-negotiable):
//!
//! * The deterministic engine is authoritative. Assistance never creates,
//!   removes, or overrides findings.
//! * Assistance is **off by default** and never performs network access on its
//!   own.
//! * Any assistant output is clearly labeled and is derived only from existing
//!   findings and rule metadata. It cannot invent source locations.
//! * Enabling a model-backed provider without a configured, allowlisted provider
//!   fails closed.
//!
//! This module ships an offline, rule-based guidance assistant. A model-backed
//! assistant can be added by implementing [`Assistant`]; it must obey the same
//! constraints and the network policy in `docs/security/security-model.md`.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::category::Category;
use crate::finding::Finding;
use crate::rule::RuleMetadata;
use crate::severity::Severity;

mod cache;
mod redaction;

pub use cache::{cache_key, AssistanceCache, CachedAssistant, CACHE_DIR, DEFAULT_MAX_ENTRIES};
pub use redaction::{redact, HOME, REDACTED};

#[cfg(feature = "ai-http")]
pub mod http;

/// Label prepended to all offline (non-model) guidance.
pub const OFFLINE_LABEL: &str = "GUIDANCE (offline, rule-based)";

/// Label prepended to model-generated text.
pub const AI_LABEL: &str = "AI-GENERATED ASSISTANCE (not authoritative)";

/// Conservative default sampling temperature (deterministic assistance).
pub const DEFAULT_TEMPERATURE: f32 = 0.0;
/// Highest accepted sampling temperature.
pub const MAX_TEMPERATURE: f32 = 1.0;
/// Conservative default output-token budget.
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 1024;
/// Lowest accepted output-token budget.
pub const MIN_MAX_OUTPUT_TOKENS: u32 = 1;
/// Highest accepted output-token budget.
pub const MAX_MAX_OUTPUT_TOKENS: u32 = 8192;

/// Configuration for the optional assistance layer.
///
/// Defaults are deliberately conservative: assistance is disabled, sampling is
/// deterministic, and the output budget is small. Out-of-range values are
/// rejected by [`AiConfig::validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AiConfig {
    /// Master switch. Disabled by default.
    pub enabled: bool,
    /// Provider name (for example `openai`). No provider is bundled.
    pub provider: Option<String>,
    /// Model identifier.
    pub model: Option<String>,
    /// Sampling temperature in `0.0..=1.0`. Defaults to `0.0`.
    pub temperature: f32,
    /// Maximum tokens the provider may return, in `1..=8192`.
    ///
    /// Defaults to `1024`.
    pub max_output_tokens: u32,
    /// Maximum provider requests per scan. `None` means unlimited. The default
    /// is deliberately finite so a scan with thousands of findings cannot
    /// trigger thousands of requests.
    pub max_requests: Option<usize>,
    /// Maximum retries for a single transient provider failure. Retries are
    /// bounded; assistance never retries indefinitely.
    pub max_retries: usize,
    /// Base exponential backoff delay in milliseconds.
    pub backoff_ms: u64,
    /// Upper bound for a single backoff delay in milliseconds.
    pub max_backoff_ms: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            enabled: false,
            provider: None,
            model: None,
            temperature: DEFAULT_TEMPERATURE,
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
            max_requests: Some(DEFAULT_MAX_REQUESTS),
            max_retries: DEFAULT_MAX_RETRIES,
            backoff_ms: DEFAULT_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
        }
    }
}

/// Validation errors for [`AiConfig`].
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AiConfigError {
    /// Temperature is not a finite value inside the accepted range.
    #[error("ai.temperature must be finite and between 0.0 and {max} (got {value})")]
    Temperature {
        /// Rejected value.
        value: f32,
        /// Highest accepted value.
        max: f32,
    },
    /// Output-token budget is outside the accepted range.
    #[error("ai.max_output_tokens must be between {min} and {max} (got {value})")]
    MaxOutputTokens {
        /// Rejected value.
        value: u32,
        /// Lowest accepted value.
        min: u32,
        /// Highest accepted value.
        max: u32,
    },
}

impl AiConfig {
    /// Validates the model parameters against their safety bounds.
    pub fn validate(&self) -> Result<(), AiConfigError> {
        if !self.temperature.is_finite()
            || self.temperature < 0.0
            || self.temperature > MAX_TEMPERATURE
        {
            return Err(AiConfigError::Temperature {
                value: self.temperature,
                max: MAX_TEMPERATURE,
            });
        }
        if !(MIN_MAX_OUTPUT_TOKENS..=MAX_MAX_OUTPUT_TOKENS).contains(&self.max_output_tokens) {
            return Err(AiConfigError::MaxOutputTokens {
                value: self.max_output_tokens,
                min: MIN_MAX_OUTPUT_TOKENS,
                max: MAX_MAX_OUTPUT_TOKENS,
            });
        }
        Ok(())
    }
}

/// Default provider request budget for one scan.
pub const DEFAULT_MAX_REQUESTS: usize = 50;
/// Default retry budget for one provider request.
pub const DEFAULT_MAX_RETRIES: usize = 2;
/// Default base backoff delay.
pub const DEFAULT_BACKOFF_MS: u64 = 250;
/// Default maximum backoff delay.
pub const DEFAULT_MAX_BACKOFF_MS: u64 = 4_000;

/// Errors from the assistance layer.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AiError {
    /// Assistance was enabled but no provider is configured.
    #[error("assistance is enabled but no provider is configured")]
    NotConfigured,
    /// A provider was named but is not available in this build.
    #[error("assistance provider '{0}' is not available in this build")]
    ProviderUnavailable(String),
    /// A provider returned text that referenced findings it was not given.
    #[error("provider response rejected: {0}")]
    InvalidResponse(String),
    /// The configured provider request budget was exhausted.
    #[error("assistance request budget exhausted after {0} request(s)")]
    RequestBudgetExhausted(usize),
}

/// Bounded request budget and retry policy for provider calls.
///
/// The budget is per scan and fail-closed: once exhausted, the caller continues
/// without assistance rather than issuing more requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallPolicy {
    max_requests: Option<usize>,
    used: usize,
    max_retries: usize,
    backoff: Duration,
    max_backoff: Duration,
}

impl CallPolicy {
    /// Builds a policy from configuration.
    pub fn from_config(config: &AiConfig) -> Self {
        CallPolicy {
            max_requests: config.max_requests,
            used: 0,
            max_retries: config.max_retries,
            backoff: Duration::from_millis(config.backoff_ms),
            max_backoff: Duration::from_millis(config.max_backoff_ms.max(config.backoff_ms)),
        }
    }

    /// Attempts to consume one request slot.
    pub fn try_acquire(&mut self) -> Result<(), AiError> {
        if let Some(limit) = self.max_requests {
            if self.used >= limit {
                return Err(AiError::RequestBudgetExhausted(limit));
            }
        }
        self.used += 1;
        Ok(())
    }

    /// Requests consumed so far.
    pub fn used(&self) -> usize {
        self.used
    }

    /// Configured request limit, if any.
    pub fn limit(&self) -> Option<usize> {
        self.max_requests
    }

    /// Delays for retry attempts, capped at `max_backoff_ms`.
    ///
    /// `attempt` is 1-based. The delay doubles each attempt so a transient
    /// outage is not hammered, but it never exceeds the configured cap.
    pub fn backoff_delay(&self, attempt: usize) -> Duration {
        let exponent = attempt.saturating_sub(1).min(16) as u32;
        let millis = self
            .backoff
            .as_millis()
            .saturating_mul(1u128 << exponent)
            .min(self.max_backoff.as_millis());
        Duration::from_millis(millis.min(u64::MAX as u128) as u64)
    }

    /// Runs `operation` with a bounded number of exponential-backoff retries.
    ///
    /// `operation` receives the 1-based attempt number. The final error is
    /// returned once the retry budget is spent; callers must fail closed.
    pub fn run_with_retry<T, E, F>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut(usize) -> Result<T, E>,
    {
        let mut attempt = 1;
        loop {
            match operation(attempt) {
                Ok(value) => return Ok(value),
                Err(error) => {
                    if attempt > self.max_retries {
                        return Err(error);
                    }
                    std::thread::sleep(self.backoff_delay(attempt));
                    attempt += 1;
                }
            }
        }
    }
}

/// A bounded session around an [`Assistant`].
///
/// Every summarize/explain call consumes one request slot. When the budget is
/// exhausted the session returns `None` and records that fact, so the CLI can
/// emit a diagnostic and continue the scan without further provider calls.
pub struct GuidanceSession<'a> {
    assistant: &'a dyn Assistant,
    policy: CallPolicy,
    exhausted: bool,
}

impl<'a> GuidanceSession<'a> {
    /// Wraps an assistant with the configured request policy.
    pub fn new(assistant: &'a dyn Assistant, config: &AiConfig) -> Self {
        GuidanceSession {
            assistant,
            policy: CallPolicy::from_config(config),
            exhausted: false,
        }
    }

    /// Summarizes findings if a request slot is available.
    pub fn summarize(&mut self, findings: &[Finding]) -> Option<Explanation> {
        self.call(|assistant| assistant.summarize(findings))
    }

    /// Explains one finding if a request slot is available.
    pub fn explain_finding(
        &mut self,
        finding: &Finding,
        rule: Option<&RuleMetadata>,
    ) -> Option<Explanation> {
        self.call(|assistant| assistant.explain_finding(finding, rule))
    }

    /// Whether a call was skipped because the request budget was exhausted.
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    /// Requests consumed so far.
    pub fn requests_used(&self) -> usize {
        self.policy.used()
    }

    fn call<F>(&mut self, operation: F) -> Option<Explanation>
    where
        F: FnOnce(&dyn Assistant) -> Explanation,
    {
        if self.policy.try_acquire().is_err() {
            self.exhausted = true;
            return None;
        }
        Some(operation(self.assistant))
    }
}

/// A clearly labeled piece of assistance text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    /// The text.
    pub text: String,
    /// Whether the text came from a model (true) or from offline rules (false).
    pub ai_generated: bool,
    /// Human-readable label placed before the text.
    pub label: &'static str,
}

impl Explanation {
    /// Creates an explanation with its text redacted.
    pub fn new(text: impl Into<String>, ai_generated: bool, label: &'static str) -> Self {
        Explanation {
            text: redact(&text.into()),
            ai_generated,
            label,
        }
    }

    /// Renders the label and text.
    ///
    /// Redaction is applied again here so a provider that constructs
    /// [`Explanation`] directly still cannot emit credentials or home paths.
    pub fn render(&self) -> String {
        format!("{}\n\n{}", self.label, redact(&self.text))
    }
}

/// Factory for a named assistance provider.
pub type ProviderFactory = fn() -> Box<dyn Assistant>;

/// Registry of provider factories keyed by name.
///
/// Providers are opt-in and never registered implicitly. Unknown names fail
/// closed, so configuration alone cannot enable an unvetted provider.
#[derive(Default)]
pub struct ProviderRegistry {
    factories: BTreeMap<String, ProviderFactory>,
}

impl ProviderRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the built-in registry. Only offline providers are registered by
    /// default; model-backed providers must be added by their feature module.
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        registry.register("rule-guidance", || Box::new(RuleGuidance));
        registry
    }

    /// Registers a factory, replacing any existing entry with the same name.
    pub fn register(&mut self, name: impl Into<String>, factory: ProviderFactory) {
        self.factories.insert(name.into(), factory);
    }

    /// Resolves a provider by name, failing closed when it is unknown.
    pub fn build(&self, name: &str) -> Result<Box<dyn Assistant>, AiError> {
        self.factories
            .get(name)
            .map(|factory| factory())
            .ok_or_else(|| AiError::ProviderUnavailable(name.to_string()))
    }

    /// Registered provider names, sorted for stable output.
    pub fn names(&self) -> Vec<&str> {
        self.factories.keys().map(String::as_str).collect()
    }
}

/// Validates that assistance text only references findings already reported.
///
/// Any `SS-###` id that is not in `findings` is rejected. This keeps the
/// deterministic engine authoritative even if a provider is hostile or buggy.
pub fn validate_explanation(
    explanation: &Explanation,
    findings: &[Finding],
) -> Result<(), AiError> {
    let known: std::collections::BTreeSet<&str> = findings
        .iter()
        .map(|finding| finding.rule_id.as_str())
        .collect();
    for token in explanation
        .text
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
    {
        if token.len() == 6
            && token.starts_with("SS-")
            && token[3..].chars().all(|c| c.is_ascii_digit())
            && !known.contains(token)
        {
            return Err(AiError::InvalidResponse(format!(
                "references unknown finding {token}"
            )));
        }
    }
    Ok(())
}
/// An assistance provider.
///
/// Implementations must never fabricate findings or locations, and must never
/// claim certainty beyond the underlying evidence.
pub trait Assistant {
    /// Provider name.
    fn name(&self) -> &str;

    /// Explains a finding using its existing evidence and rule metadata.
    fn explain_finding(&self, finding: &Finding, rule: Option<&RuleMetadata>) -> Explanation;

    /// Summarizes a scan.
    fn summarize(&self, findings: &[Finding]) -> Explanation;
}

/// Offline, deterministic, rule-based guidance.
#[derive(Debug, Default, Clone, Copy)]
pub struct RuleGuidance;

impl Assistant for RuleGuidance {
    fn name(&self) -> &str {
        "rule-guidance"
    }

    fn explain_finding(&self, finding: &Finding, rule: Option<&RuleMetadata>) -> Explanation {
        let mut text = String::new();
        text.push_str(&format!(
            "{} at {} (severity: {}, confidence: {}).\n\n",
            finding.rule_id,
            finding.location.display_label(),
            finding.severity,
            finding.confidence
        ));
        text.push_str(&format!("What it means: {}\n\n", finding.description));
        text.push_str(&format!("Evidence observed: {}\n\n", finding.evidence));
        text.push_str(&format!(
            "Suggested remediation: {}\n\n",
            finding.remediation
        ));
        if finding.severity >= Severity::High
            && finding.confidence <= crate::confidence::Confidence::Medium
        {
            text.push_str(
                "Note: this is a high-severity, heuristic finding. Review the code \
before treating it as a confirmed vulnerability.\n\n",
            );
        }
        if let Some(rule) = rule {
            if !rule.references.is_empty() {
                text.push_str("References:\n");
                for reference in rule.references {
                    text.push_str(&format!("- {reference}\n"));
                }
                text.push('\n');
            }
        }
        text.push_str(
            "This guidance is derived from the rule documentation and the finding \
evidence. It is not an independent security judgment and does not change the \
deterministic result.",
        );

        Explanation::new(text, false, OFFLINE_LABEL)
    }

    fn summarize(&self, findings: &[Finding]) -> Explanation {
        let mut counts = [0usize; 5];
        let mut categories: Vec<Category> = Vec::new();
        for finding in findings {
            counts[finding.severity.rank() as usize] += 1;
            if !categories.contains(&finding.category) {
                categories.push(finding.category);
            }
        }
        categories.sort();

        let mut text = format!(
            "Scanned with {} deterministic findings: critical {}, high {}, medium {}, low {}, info {}.\n\n",
            findings.len(),
            counts[4],
            counts[3],
            counts[2],
            counts[1],
            counts[0]
        );
        if !categories.is_empty() {
            text.push_str("Categories observed: ");
            text.push_str(
                &categories
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            text.push_str(".\n\n");
        }
        text.push_str(
            "Prioritize critical and high findings first, and treat low-confidence \
findings as review prompts rather than confirmed vulnerabilities.",
        );

        Explanation::new(text, false, OFFLINE_LABEL)
    }
}

/// Builds the configured assistant.
///
/// Returns offline [`RuleGuidance`] when assistance is disabled. When assistance
/// is enabled, it fails closed unless a bundled provider is available (none is,
/// by design, until a provider is implemented with the required safeguards).
pub fn assistant(config: &AiConfig) -> Result<RuleGuidance, AiError> {
    if !config.enabled {
        return Ok(RuleGuidance);
    }
    match &config.provider {
        None => Err(AiError::NotConfigured),
        Some(provider) => Err(AiError::ProviderUnavailable(provider.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::Category;
    use crate::confidence::Confidence;
    use crate::finding::SourceLocation;
    use crate::rule::RuleMetadata;
    use crate::severity::Severity;

    fn meta() -> RuleMetadata {
        RuleMetadata::new(
            "SS-001",
            "Title",
            "description of the problem with enough characters",
            Category::Authorization,
            Severity::High,
            Confidence::Medium,
            "call require_auth",
        )
        .with_references(&["https://example.com"])
    }

    fn finding() -> Finding {
        Finding::new(
            &meta(),
            SourceLocation::new("src/lib.rs", 5),
            "evidence here",
        )
    }

    #[test]
    fn disabled_by_default_returns_offline_guidance() {
        let config = AiConfig::default();
        assert!(!config.enabled);
        assert!(assistant(&config).is_ok());
    }

    #[test]
    fn default_model_parameters_are_conservative_and_valid() {
        let config = AiConfig::default();
        assert_eq!(config.temperature, DEFAULT_TEMPERATURE);
        assert_eq!(config.max_output_tokens, DEFAULT_MAX_OUTPUT_TOKENS);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn accepts_temperature_and_token_boundaries() {
        for temperature in [0.0, 0.5, MAX_TEMPERATURE] {
            let config = AiConfig {
                temperature,
                ..AiConfig::default()
            };
            assert!(config.validate().is_ok(), "rejected {temperature}");
        }
        for max_output_tokens in [MIN_MAX_OUTPUT_TOKENS, 1024, MAX_MAX_OUTPUT_TOKENS] {
            let config = AiConfig {
                max_output_tokens,
                ..AiConfig::default()
            };
            assert!(config.validate().is_ok(), "rejected {max_output_tokens}");
        }
    }

    #[test]
    fn rejects_unsafe_temperature_values() {
        for temperature in [-0.1, 1.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let config = AiConfig {
                temperature,
                ..AiConfig::default()
            };
            assert!(
                matches!(config.validate(), Err(AiConfigError::Temperature { .. })),
                "accepted {temperature}"
            );
        }
    }

    #[test]
    fn rejects_unsafe_output_token_limits() {
        for max_output_tokens in [0, MAX_MAX_OUTPUT_TOKENS + 1, u32::MAX] {
            let config = AiConfig {
                max_output_tokens,
                ..AiConfig::default()
            };
            assert!(
                matches!(
                    config.validate(),
                    Err(AiConfigError::MaxOutputTokens { .. })
                ),
                "accepted {max_output_tokens}"
            );
        }
    }

    #[test]
    fn enabling_without_provider_fails_closed() {
        let config = AiConfig {
            enabled: true,
            ..AiConfig::default()
        };
        assert_eq!(assistant(&config).unwrap_err(), AiError::NotConfigured);
    }

    #[test]
    fn enabling_with_unknown_provider_fails_closed() {
        let config = AiConfig {
            enabled: true,
            provider: Some("no-such-provider".into()),
            ..AiConfig::default()
        };
        assert!(matches!(
            assistant(&config),
            Err(AiError::ProviderUnavailable(_))
        ));
    }

    #[test]
    fn guidance_is_labeled_and_uses_existing_evidence() {
        let explanation = RuleGuidance.explain_finding(&finding(), Some(&meta()));
        assert!(!explanation.ai_generated);
        assert_eq!(explanation.label, OFFLINE_LABEL);
        assert!(explanation.text.contains("evidence here"));
        assert!(explanation.text.contains("src/lib.rs:5"));
        assert!(explanation.text.contains("call require_auth"));
    }

    #[test]
    fn summary_reports_existing_findings_only() {
        let findings = vec![finding()];
        let explanation = RuleGuidance.summarize(&findings);
        assert!(explanation.text.contains("1 deterministic findings"));
        assert!(explanation.text.contains("critical 0, high 1"));
    }

    #[test]
    fn offline_guidance_redacts_secrets_and_home_paths() {
        let mut finding = finding();
        finding.description =
            "leaked OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz and ghp_abcdefghijklmnopqrstuvwxyz0123456789"
                .to_string();
        finding.evidence = "password = \"hunter2\"".to_string();
        finding.remediation = "move /home/alice/project out of the prompt".to_string();
        finding.location.file =
            std::path::PathBuf::from("C:\\Users\\alice\\contracts\\src\\lib.rs");

        let rendered = RuleGuidance
            .explain_finding(&finding, Some(&meta()))
            .render();
        for secret in [
            "sk-abcdefghijklmnopqrstuvwxyz",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "hunter2",
            "alice",
        ] {
            assert!(!rendered.contains(secret), "leaked {secret}: {rendered}");
        }
        assert!(rendered.contains(REDACTED), "{rendered}");
        assert!(rendered.contains(HOME), "{rendered}");
    }

    struct LeakyProvider;

    impl Assistant for LeakyProvider {
        fn name(&self) -> &str {
            "leaky"
        }

        fn explain_finding(&self, _finding: &Finding, _rule: Option<&RuleMetadata>) -> Explanation {
            // A provider could construct Explanation without the constructor;
            // rendering must still be a final redaction gate.
            Explanation {
                text: "token ghp_abcdefghijklmnopqrstuvwxyz0123456789 at /home/alice/project"
                    .to_string(),
                ai_generated: true,
                label: AI_LABEL,
            }
        }

        fn summarize(&self, _findings: &[Finding]) -> Explanation {
            self.explain_finding(&finding(), None)
        }
    }

    #[test]
    fn provider_output_is_redacted_at_render_time() {
        let rendered = LeakyProvider.explain_finding(&finding(), None).render();
        assert!(!rendered.contains("ghp_"), "{rendered}");
        assert!(!rendered.contains("/home/alice"), "{rendered}");
        assert!(rendered.contains(REDACTED), "{rendered}");
        assert!(rendered.contains(HOME), "{rendered}");
    }

    #[test]
    fn registry_resolves_named_provider_and_fails_closed() {
        let registry = ProviderRegistry::builtin();
        assert_eq!(registry.names(), vec!["rule-guidance"]);
        assert!(registry.build("rule-guidance").is_ok());
        assert_eq!(
            registry.build("unknown").err(),
            Some(AiError::ProviderUnavailable("unknown".to_string()))
        );
    }

    #[test]
    fn validator_rejects_unknown_findings_and_accepts_known_ones() {
        let known = vec![finding()];
        let ok = Explanation::new("SS-001 is described here.", false, OFFLINE_LABEL);
        assert!(validate_explanation(&ok, &known).is_ok());
        let hostile = Explanation::new("Also SS-099 at src/lib.rs:10", true, AI_LABEL);
        assert!(matches!(
            validate_explanation(&hostile, &known),
            Err(AiError::InvalidResponse(_))
        ));
    }

    struct CountingAssistant {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl CountingAssistant {
        fn new() -> Self {
            CountingAssistant {
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl Assistant for CountingAssistant {
        fn name(&self) -> &str {
            "counting"
        }

        fn explain_finding(&self, _finding: &Finding, _rule: Option<&RuleMetadata>) -> Explanation {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Explanation::new("explained", true, AI_LABEL)
        }

        fn summarize(&self, _findings: &[Finding]) -> Explanation {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Explanation::new("summary", true, AI_LABEL)
        }
    }

    #[test]
    fn request_budget_stops_further_calls_and_reports_exhaustion() {
        let assistant = CountingAssistant::new();
        let config = AiConfig {
            enabled: true,
            max_requests: Some(2),
            ..AiConfig::default()
        };
        let mut session = GuidanceSession::new(&assistant, &config);

        assert!(session.summarize(&[finding()]).is_some());
        assert!(session.explain_finding(&finding(), Some(&meta())).is_some());
        assert!(session.explain_finding(&finding(), Some(&meta())).is_none());
        assert!(session.exhausted());
        assert_eq!(session.requests_used(), 2);
        assert_eq!(assistant.calls(), 2);
    }

    #[test]
    fn zero_request_budget_fails_closed() {
        let assistant = CountingAssistant::new();
        let config = AiConfig {
            enabled: true,
            max_requests: Some(0),
            ..AiConfig::default()
        };
        let mut session = GuidanceSession::new(&assistant, &config);
        assert!(session.summarize(&[]).is_none());
        assert!(session.exhausted());
        assert_eq!(assistant.calls(), 0);
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        let config = AiConfig {
            backoff_ms: 100,
            max_backoff_ms: 250,
            ..AiConfig::default()
        };
        let policy = CallPolicy::from_config(&config);
        assert_eq!(policy.backoff_delay(1), Duration::from_millis(100));
        assert_eq!(policy.backoff_delay(2), Duration::from_millis(200));
        assert_eq!(policy.backoff_delay(3), Duration::from_millis(250));
        assert_eq!(policy.backoff_delay(99), Duration::from_millis(250));
    }

    #[test]
    fn retries_are_bounded_and_eventually_succeed() {
        let config = AiConfig {
            max_retries: 2,
            backoff_ms: 1,
            max_backoff_ms: 2,
            ..AiConfig::default()
        };
        let policy = CallPolicy::from_config(&config);
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let result: Result<&str, &str> = policy.run_with_retry(|attempt| {
            attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if attempt < 3 {
                Err("transient")
            } else {
                Ok("done")
            }
        });
        assert_eq!(result, Ok("done"));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
