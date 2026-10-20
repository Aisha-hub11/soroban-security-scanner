use std::path::PathBuf;

use soroban_scan_core::config::ScanConfig;
use soroban_scan_core::engine;
use soroban_scan_core::registry::RuleRegistry;
use soroban_scan_core::report::render_junit;

#[test]
fn junit_maps_findings_to_testcases_and_escapes_xml() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/corpus/SS-001/positive.rs");
    let source = std::fs::read_to_string(path).unwrap();
    let registry = RuleRegistry::with_default_rules();
    let outcome =
        engine::scan_source_str(&source, "positive.rs", &ScanConfig::default(), &registry);
    assert!(!outcome.findings.is_empty());

    let xml = render_junit(&outcome);
    assert!(xml.starts_with("<?xml version=\"1.0\""));
    assert!(xml.contains("<testsuites"));
    assert_eq!(xml.matches("<testcase ").count(), outcome.findings.len());
    assert!(xml.contains("<failure "));
    assert!(xml.contains("SS-001"));
    assert!(!xml.contains(" & "));
}
