use soroban_scan_core::config::ScanConfig;
use soroban_scan_core::engine;
use soroban_scan_core::registry::RuleRegistry;
use soroban_scan_core::report;

const SOURCE: &str = r#"
#[contractimpl]
impl Demo {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let key = symbol_short!("BAL");
        env.storage().persistent().set(&key, &amount);
        env.storage().persistent().extend_ttl(&key, 100, 1000);
    }
}
"#;

fn scan() -> engine::ScanOutcome {
    let registry = RuleRegistry::with_default_rules();
    engine::scan_source_str(SOURCE, "src/lib.rs", &ScanConfig::default(), &registry)
}

#[test]
fn category_counts_match_the_gate_findings() {
    let outcome = scan();
    assert!(
        !outcome.findings.is_empty(),
        "the sample should trigger rules"
    );

    let total: usize = outcome.stats.findings_by_category.values().sum();
    assert_eq!(total, outcome.findings.len());

    for (category, count) in &outcome.stats.findings_by_category {
        let expected = outcome
            .findings
            .iter()
            .filter(|finding| finding.category.as_str() == category)
            .count();
        assert_eq!(*count, expected, "count mismatch for {category}");
    }
}

#[test]
fn category_counts_are_sorted_and_serialized() {
    let outcome = scan();
    let keys: Vec<_> = outcome.stats.findings_by_category.keys().cloned().collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "category keys must be deterministic");

    let json = report::render_json(&outcome).expect("json report");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let serialized = value["stats"]["findings_by_category"]
        .as_object()
        .expect("findings_by_category object");

    assert_eq!(serialized.len(), outcome.stats.findings_by_category.len());
    for (category, count) in &outcome.stats.findings_by_category {
        assert_eq!(serialized[category].as_u64().unwrap(), *count as u64);
    }

    let serialized_keys: Vec<_> = serialized.keys().cloned().collect();
    let mut sorted_serialized = serialized_keys.clone();
    sorted_serialized.sort();
    assert_eq!(serialized_keys, sorted_serialized);
}
