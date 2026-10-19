//! Content-addressed cache for assistance output (#2).
//!
//! Repeated assistance requests for the same finding waste latency and, with a
//! model-backed provider, tokens. This module adds a small cache keyed by
//! **fingerprint + provider + model**:
//!
//! * the key is a SHA-256 digest, so it is safe as a filename and never embeds
//!   source text, evidence, or configuration;
//! * the cache stores only the rendered [`Explanation`] (text, label kind, and
//!   whether it was model generated); it never stores source files, secrets,
//!   environment variables, or request payloads;
//! * storage is bounded to [`DEFAULT_MAX_ENTRIES`] by default and evicts the
//!   least-recently-used entry deterministically (oldest access first, ties by
//!   key);
//! * entries live under the scan cache directory (`.scanner-cache/ai`), which
//!   discovery already excludes from analysis.
//!
//! [`CachedAssistant`] wraps any [`Assistant`] implementation, so a future
//! model provider gets caching without changing its own code.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Assistant, Explanation, AI_LABEL, OFFLINE_LABEL};

/// Default maximum number of cached explanations.
pub const DEFAULT_MAX_ENTRIES: usize = 256;

/// Directory (relative to the scan root) where assistance entries are stored.
pub const CACHE_DIR: &str = ".scanner-cache/ai";

const INDEX_FILE: &str = "index.json";
const KEY_PREFIX: &str = "assistance-cache-v1";

/// Computes the content-addressed cache key for a finding/provider/model tuple.
///
/// The digest is stable across processes and platforms. Provider and model are
/// part of the key so switching either never reuses an incompatible answer.
pub fn cache_key(fingerprint: &str, provider: &str, model: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(KEY_PREFIX.as_bytes());
    hasher.update(b"\0");
    hasher.update(fingerprint.as_bytes());
    hasher.update(b"\0");
    hasher.update(provider.as_bytes());
    hasher.update(b"\0");
    hasher.update(model.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LabelKind {
    Offline,
    Ai,
}

impl LabelKind {
    fn from_label(label: &str) -> Self {
        if label == OFFLINE_LABEL {
            LabelKind::Offline
        } else {
            LabelKind::Ai
        }
    }

    fn label(self) -> &'static str {
        match self {
            LabelKind::Offline => OFFLINE_LABEL,
            LabelKind::Ai => AI_LABEL,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedExplanation {
    text: String,
    ai_generated: bool,
    label: LabelKind,
}

impl CachedExplanation {
    fn from_explanation(explanation: &Explanation) -> Self {
        CachedExplanation {
            text: explanation.text.clone(),
            ai_generated: explanation.ai_generated,
            label: LabelKind::from_label(explanation.label),
        }
    }

    fn into_explanation(self) -> Explanation {
        Explanation {
            text: self.text,
            ai_generated: self.ai_generated,
            label: self.label.label(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheIndex {
    tick: u64,
    /// Cache key -> last-used tick.
    entries: BTreeMap<String, u64>,
}

/// A bounded, file-backed cache of assistance explanations.
#[derive(Debug, Clone)]
pub struct AssistanceCache {
    root: PathBuf,
    capacity: usize,
}

impl AssistanceCache {
    /// Creates a cache rooted at `root`, bounded to `capacity` entries.
    ///
    /// A capacity of zero is treated as one so the cache always makes progress.
    pub fn new(root: impl Into<PathBuf>, capacity: usize) -> Self {
        AssistanceCache {
            root: root.into(),
            capacity: capacity.max(1),
        }
    }

    /// Cache root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Maximum number of entries retained.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of tracked entries (stale entries are pruned by [`Self::get`]).
    pub fn len(&self) -> usize {
        self.read_index().entries.len()
    }

    /// Returns true when no entries are tracked.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Looks up an explanation by key and marks it as recently used.
    ///
    /// Missing or corrupt entries are treated as misses. Reading never fails a
    /// caller: assistance must work even when the cache directory is unusable.
    pub fn get(&self, key: &str) -> Option<Explanation> {
        let entry: CachedExplanation = read_json(&self.entry_path(key))?;
        let mut index = self.read_index();
        index.tick = index.tick.saturating_add(1);
        index.entries.insert(key.to_string(), index.tick);
        // Best-effort: a failed touch must not turn a cache hit into an error.
        let _ = self.write_index(&index);
        Some(entry.into_explanation())
    }

    /// Stores an explanation, evicting the least-recently-used entry when full.
    pub fn put(&self, key: &str, explanation: &Explanation) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let mut index = self.read_index();

        if !index.entries.contains_key(key) {
            while index.entries.len() >= self.capacity {
                let Some(victim) = index
                    .entries
                    .iter()
                    .min_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.cmp(b.0)))
                    .map(|(key, _)| key.clone())
                else {
                    break;
                };
                index.entries.remove(&victim);
                let _ = fs::remove_file(self.entry_path(&victim));
            }
        }

        let entry = CachedExplanation::from_explanation(explanation);
        write_json(&self.entry_path(key), &entry)?;
        index.tick = index.tick.saturating_add(1);
        index.entries.insert(key.to_string(), index.tick);
        self.write_index(&index)
    }

    /// Removes every cached entry.
    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.root) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn entry_path(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.json"))
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(INDEX_FILE)
    }

    fn read_index(&self) -> CacheIndex {
        read_json(&self.index_path()).unwrap_or_default()
    }

    fn write_index(&self, index: &CacheIndex) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        write_json(&self.index_path(), index)
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(path, bytes)
}

/// An [`Assistant`] decorator that caches per-finding explanations.
///
/// `summarize` is intentionally not cached: summaries cover a whole finding set
/// and are cheap to recompute, while the expensive unit is one finding.
pub struct CachedAssistant<A: Assistant> {
    inner: A,
    cache: AssistanceCache,
    provider: String,
    model: String,
}

impl<A: Assistant> CachedAssistant<A> {
    /// Wraps `inner`, caching explanations under `cache`.
    pub fn new(
        inner: A,
        cache: AssistanceCache,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        CachedAssistant {
            inner,
            cache,
            provider: provider.into(),
            model: model.into(),
        }
    }

    /// The underlying cache.
    pub fn cache(&self) -> &AssistanceCache {
        &self.cache
    }

    /// Provider name used in cache keys.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Model identifier used in cache keys.
    pub fn model(&self) -> &str {
        &self.model
    }
}

impl<A: Assistant> Assistant for CachedAssistant<A> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn explain_finding(
        &self,
        finding: &crate::finding::Finding,
        rule: Option<&crate::rule::RuleMetadata>,
    ) -> Explanation {
        let key = cache_key(&finding.fingerprint, &self.provider, &self.model);
        if let Some(cached) = self.cache.get(&key) {
            return cached;
        }
        let explanation = self.inner.explain_finding(finding, rule);
        let _ = self.cache.put(&key, &explanation);
        explanation
    }

    fn summarize(&self, findings: &[crate::finding::Finding]) -> Explanation {
        self.inner.summarize(findings)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::category::Category;
    use crate::confidence::Confidence;
    use crate::finding::{Finding, SourceLocation};
    use crate::rule::RuleMetadata;
    use crate::severity::Severity;

    fn temp_cache(name: &str, capacity: usize) -> AssistanceCache {
        let root = std::env::temp_dir().join(format!(
            "soroban-scan-ai-cache-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        AssistanceCache::new(root, capacity)
    }

    fn finding() -> Finding {
        let meta = RuleMetadata::new(
            "SS-001",
            "Title",
            "description of the problem with enough characters",
            Category::Authorization,
            Severity::High,
            Confidence::Medium,
            "call require_auth",
        );
        Finding::new(&meta, SourceLocation::new("src/lib.rs", 5), "evidence here")
    }

    fn explanation(text: &str) -> Explanation {
        Explanation {
            text: text.to_string(),
            ai_generated: true,
            label: AI_LABEL,
        }
    }

    #[test]
    fn cache_key_is_stable_and_scoped_to_provider_and_model() {
        let base = cache_key("fingerprint", "provider-a", "model-a");

        assert_eq!(base, cache_key("fingerprint", "provider-a", "model-a"));
        assert_ne!(
            base,
            cache_key("other-fingerprint", "provider-a", "model-a")
        );
        assert_ne!(base, cache_key("fingerprint", "provider-b", "model-a"));
        assert_ne!(base, cache_key("fingerprint", "provider-a", "model-b"));
        assert_eq!(base.len(), 64);
    }

    #[test]
    fn round_trips_explanations() {
        let cache = temp_cache("round-trip", 8);
        let key = cache_key("fp", "offline", "default");
        let original = explanation("the explanation");

        cache.put(&key, &original).unwrap();
        let cached = cache.get(&key).expect("cache hit");

        assert_eq!(cached.text, original.text);
        assert_eq!(cached.ai_generated, original.ai_generated);
        assert_eq!(cached.label, AI_LABEL);
        assert_eq!(cache.len(), 1);

        cache.clear().unwrap();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn evicts_the_least_recently_used_entry() {
        let cache = temp_cache("lru", 2);
        let a = cache_key("a", "p", "m");
        let b = cache_key("b", "p", "m");
        let c = cache_key("c", "p", "m");

        cache.put(&a, &explanation("a")).unwrap();
        cache.put(&b, &explanation("b")).unwrap();
        // Touch `a` so `b` becomes the least-recently-used entry.
        assert!(cache.get(&a).is_some());
        cache.put(&c, &explanation("c")).unwrap();

        assert!(cache.get(&a).is_some());
        assert!(cache.get(&b).is_none(), "b should have been evicted");
        assert!(cache.get(&c).is_some());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn corrupt_entries_are_misses() {
        let cache = temp_cache("corrupt", 4);
        let key = cache_key("fp", "p", "m");
        fs::create_dir_all(cache.root()).unwrap();
        fs::write(cache.root().join(format!("{key}.json")), b"not json").unwrap();

        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn cached_assistant_skips_the_provider_on_a_hit() {
        struct CountingAssistant {
            calls: Cell<usize>,
        }

        impl Assistant for CountingAssistant {
            fn name(&self) -> &str {
                "counting"
            }

            fn explain_finding(
                &self,
                _finding: &Finding,
                _rule: Option<&RuleMetadata>,
            ) -> Explanation {
                self.calls.set(self.calls.get() + 1);
                explanation(&format!("call {}", self.calls.get()))
            }

            fn summarize(&self, _findings: &[Finding]) -> Explanation {
                explanation("summary")
            }
        }

        let counting = CountingAssistant {
            calls: Cell::new(0),
        };
        let cache = temp_cache("assistant", 8);
        let assistant = CachedAssistant::new(counting, cache, "provider", "model");
        let finding = finding();

        let first = assistant.explain_finding(&finding, None);
        let second = assistant.explain_finding(&finding, None);

        assert_eq!(assistant.cache().len(), 1);
        assert_eq!(first.text, second.text);
        // The wrapped assistant exposed only one call via the cached text.
        assert_eq!(first.text, "call 1");
    }

    #[test]
    fn cached_assistant_separates_providers() {
        struct CountingAssistant {
            calls: Cell<usize>,
        }

        impl Assistant for CountingAssistant {
            fn name(&self) -> &str {
                "counting"
            }

            fn explain_finding(
                &self,
                _finding: &Finding,
                _rule: Option<&RuleMetadata>,
            ) -> Explanation {
                self.calls.set(self.calls.get() + 1);
                explanation(&format!("call {}", self.calls.get()))
            }

            fn summarize(&self, _findings: &[Finding]) -> Explanation {
                explanation("summary")
            }
        }

        let cache = temp_cache("provider-scope", 8);
        let finding = finding();
        let one = CachedAssistant::new(
            CountingAssistant {
                calls: Cell::new(0),
            },
            cache.clone(),
            "provider-one",
            "model",
        );
        let two = CachedAssistant::new(
            CountingAssistant {
                calls: Cell::new(0),
            },
            cache,
            "provider-two",
            "model",
        );

        assert_eq!(one.explain_finding(&finding, None).text, "call 1");
        assert_eq!(two.explain_finding(&finding, None).text, "call 1");
        assert_eq!(one.explain_finding(&finding, None).text, "call 1");
        assert_eq!(two.explain_finding(&finding, None).text, "call 1");
    }
}
