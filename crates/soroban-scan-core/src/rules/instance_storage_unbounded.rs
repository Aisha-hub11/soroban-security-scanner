//! SS-016: unbounded data stored in instance storage.

use std::collections::BTreeSet;

use crate::category::Category;
use crate::confidence::Confidence;
use crate::context::AnalysisContext;
use crate::finding::Finding;
use crate::rule::{Rule, RuleMetadata};
use crate::severity::Severity;

use super::util::{contract_entry_functions, facts_from_block, location_for};

/// Rule metadata.
pub const METADATA: RuleMetadata = RuleMetadata {
    id: "SS-016",
    title: "Unbounded data stored in instance storage",
    description: "Instance storage is loaded for every invocation and has \
strict size limits. Writing repeated distinct keys, or growing a collection, in \
instance storage increases the cost of every call and can break the contract at \
scale. Constant configuration is a better fit for instance storage.",
    category: Category::Storage,
    default_severity: Severity::Medium,
    default_confidence: Confidence::Low,
    remediation: "Keep instance storage for small, fixed configuration. Store \
growing collections in persistent storage, and bound their size explicitly.",
    references: &["https://developers.stellar.org/docs/learn/encyclopedia/storage/persisting-data"],
};

/// Detects repeated instance-storage keys and collection growth.
pub struct InstanceStorageForUnboundedData;

impl Rule for InstanceStorageForUnboundedData {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>, out: &mut Vec<Finding>) {
        if !ctx.is_confirmed_soroban() {
            return;
        }
        for source in ctx.sources() {
            let Some(file) = &source.syntax else {
                continue;
            };
            for func in contract_entry_functions(file) {
                if !func.is_public {
                    continue;
                }
                let facts = facts_from_block(func.block);
                let instance_sets: Vec<_> = facts
                    .storage_mutations()
                    .into_iter()
                    .filter(|call| {
                        call.name == "set" && call.context.to_ascii_lowercase().contains("instance")
                    })
                    .collect();

                let distinct_keys: BTreeSet<&str> = instance_sets
                    .iter()
                    .filter_map(|call| call.args.first().map(String::as_str))
                    .collect();
                if instance_sets.len() >= 2 && distinct_keys.len() >= 2 {
                    for call in instance_sets {
                        let location = location_for(ctx, source, call.line, call.column);
                        let evidence = format!(
                            "repeated instance-storage keys in `{}`: {}",
                            func.name, call.text
                        );
                        out.push(Finding::new(&METADATA, location, evidence));
                    }
                    continue;
                }

                let reads_instance = facts.method_calls.iter().any(|call| {
                    call.name == "get" && call.context.to_ascii_lowercase().contains("instance")
                });
                if !reads_instance {
                    continue;
                }
                for growth in facts.method_calls.iter().filter(|call| {
                    matches!(call.name.as_str(), "push" | "push_back" | "push_front")
                }) {
                    let location = location_for(ctx, source, growth.line, growth.column);
                    let evidence = format!(
                        "unbounded collection growth in instance storage in `{}`: {}",
                        func.name, growth.text
                    );
                    out.push(Finding::new(&METADATA, location, evidence));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::test_support::scan_rule;

    #[test]
    fn flags_repeated_distinct_instance_keys() {
        let findings = scan_rule(
            r#"
            #[contractimpl]
            impl C {
                pub fn configure(env: Env, a: u32, b: u32) {
                    env.storage().instance().set(&DataKey::A, &a);
                    env.storage().instance().set(&DataKey::B, &b);
                }
            }
            "#,
            Box::new(InstanceStorageForUnboundedData),
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule_id, "SS-016");
    }

    #[test]
    fn does_not_flag_single_instance_write() {
        let findings = scan_rule(
            r#"
            #[contractimpl]
            impl C {
                pub fn configure(env: Env, a: u32) {
                    env.storage().instance().set(&DataKey::A, &a);
                }
            }
            "#,
            Box::new(InstanceStorageForUnboundedData),
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn does_not_flag_persistent_storage() {
        let findings = scan_rule(
            r#"
            #[contractimpl]
            impl C {
                pub fn configure(env: Env, a: u32, b: u32) {
                    env.storage().persistent().set(&DataKey::A, &a);
                    env.storage().persistent().set(&DataKey::B, &b);
                }
            }
            "#,
            Box::new(InstanceStorageForUnboundedData),
        );
        assert!(findings.is_empty());
    }
}
