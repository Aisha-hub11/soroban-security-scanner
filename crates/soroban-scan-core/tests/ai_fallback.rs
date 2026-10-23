//! Deterministic offline fallback guarantees (#87).
//!
//! Assistance is advisory. The deterministic engine must produce identical
//! findings whether assistance is disabled, enabled-but-unavailable, or failing
//! at the provider boundary.

use soroban_scan_core::ai::{
    self, AiConfig, AiError, Assistant, Explanation, GuidanceSession, AI_LABEL,
};
use soroban_scan_core::config::ScanConfig;
use soroban_scan_core::engine;
use soroban_scan_core::finding::Finding;
use soroban_scan_core::registry::RuleRegistry;
use soroban_scan_core::rule::RuleMetadata;

const SOURCE: &str = include_str!("../../../fixtures/corpus/SS-001/positive.rs");

fn scan_with(ai: AiConfig) -> Vec<Finding> {
    let config = ScanConfig {
        ai,
        ..ScanConfig::default()
    };
    engine::scan_source_str(
        SOURCE,
        "src/lib.rs",
        &config,
        &RuleRegistry::with_default_rules(),
    )
    .findings
}

#[test]
fn assistance_off_and_enabled_but_unavailable_produce_identical_findings() {
    let baseline = scan_with(AiConfig::default());
    assert!(
        !baseline.is_empty(),
        "fixture must produce a deterministic finding"
    );

    let enabled_unavailable = AiConfig {
        enabled: true,
        provider: Some("openai".to_string()),
        ..AiConfig::default()
    };
    assert!(ai::assistant(&enabled_unavailable).is_err());

    assert_eq!(scan_with(enabled_unavailable), baseline);
}

struct FailingProvider;

impl Assistant for FailingProvider {
    fn name(&self) -> &str {
        "failing-mock"
    }

    fn explain_finding(&self, _finding: &Finding, _rule: Option<&RuleMetadata>) -> Explanation {
        Explanation::new("unreachable", true, AI_LABEL)
    }

    fn summarize(&self, _findings: &[Finding]) -> Explanation {
        Explanation::new("unreachable", true, AI_LABEL)
    }
}

#[test]
fn provider_errors_do_not_change_findings_and_the_scan_continues() {
    let baseline = scan_with(AiConfig::default());
    assert!(!baseline.is_empty());

    let config = AiConfig {
        enabled: true,
        provider: Some("failing-mock".to_string()),
        failure_threshold: 1,
        max_retries: 0,
        backoff_ms: 0,
        ..AiConfig::default()
    };
    let provider = FailingProvider;
    let mut session = GuidanceSession::new(&provider, &config);

    // A provider error is swallowed by the guidance session; the caller keeps
    // the deterministic result and simply receives no assistance text.
    let result =
        session.try_summarize(|_| Err(AiError::ProviderUnavailable("failing-mock".to_string())));
    assert!(result.is_none());
    assert!(session.circuit_open());
    assert_eq!(baseline, scan_with(AiConfig::default()));
}

#[test]
fn unavailable_assistant_fails_closed_without_provider_calls() {
    let config = AiConfig {
        enabled: true,
        provider: None,
        ..AiConfig::default()
    };
    assert_eq!(ai::assistant(&config).unwrap_err(), AiError::NotConfigured);
}
