//! Best-effort, per-file resolution of `use` imports.
//!
//! Detectors compare rendered path strings such as `Token::new` or
//! `Instance::set`. Those strings use the *local* names introduced by `use`
//! items, so an aliased import hides the canonical path from a detector. This
//! module builds a syntactic import map from a single file and canonicalizes
//! path strings against it.
//!
//! The map never crosses crate or module boundaries and never loads another
//! file. See `docs/architecture/import-resolution.md` for the limitations.

use std::collections::{BTreeMap, BTreeSet};

/// Result of canonicalizing a path against a file's imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Exactly one canonical path was found.
    Resolved(String),
    /// More than one candidate matches (duplicate alias or glob imports).
    Ambiguous(Vec<String>),
    /// No import owns the first path segment; the path was left as written.
    Unresolved,
}

impl Resolution {
    /// Every canonical candidate, in deterministic order.
    pub fn candidates(&self) -> Vec<&str> {
        match self {
            Resolution::Resolved(path) => vec![path],
            Resolution::Ambiguous(paths) => paths.iter().map(String::as_str).collect(),
            Resolution::Unresolved => Vec::new(),
        }
    }

    /// Whether any candidate satisfies `predicate`.
    ///
    /// Detectors use this to fail toward reporting when a glob import is
    /// ambiguous: if *any* candidate looks like the sensitive type, the call is
    /// still treated as that type.
    pub fn any_candidate(&self, predicate: impl Fn(&str) -> bool) -> bool {
        self.candidates().into_iter().any(predicate)
    }
}

/// A per-file map from local names to canonical import paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportMap {
    /// Local alias -> canonical paths. A `BTreeSet` keeps duplicate imports
    /// visible as ambiguity instead of silently picking one.
    aliases: BTreeMap<String, BTreeSet<String>>,
    /// Prefixes introduced by glob imports such as `use foo::bar::*;`.
    glob_prefixes: BTreeSet<String>,
}

impl ImportMap {
    /// Builds a map from every `use` item in a parsed file.
    ///
    /// Imports inside inline `mod` blocks are included; imports in other files
    /// are not, because analysis is per-file and explicitly syntactic.
    pub fn from_file(file: &syn::File) -> Self {
        let mut map = ImportMap::default();
        map.collect_items(&file.items);
        map
    }

    /// Builds a map from a list of items.
    pub fn from_items(items: &[syn::Item]) -> Self {
        let mut map = ImportMap::default();
        map.collect_items(items);
        map
    }

    /// Returns true when the file contains no `use` items.
    pub fn is_empty(&self) -> bool {
        self.aliases.is_empty() && self.glob_prefixes.is_empty()
    }

    /// Canonicalizes `path` against the import map.
    pub fn resolve(&self, path: &str) -> Resolution {
        let Some(segments) = normalized_segments(path) else {
            return Resolution::Unresolved;
        };
        let first = segments[0];
        let rest = &segments[1..];

        if let Some(targets) = self.aliases.get(first) {
            let candidates: BTreeSet<String> =
                targets.iter().map(|target| join(target, rest)).collect();
            return if candidates.len() == 1 {
                Resolution::Resolved(candidates.into_iter().next().unwrap_or_default())
            } else {
                Resolution::Ambiguous(candidates.into_iter().collect())
            };
        }

        // A path with more than one segment is already qualified (for example
        // `soroban_sdk::Env` or `crate::storage::State`); leave it as written.
        if segments.len() > 1 {
            return Resolution::Resolved(segments.join("::"));
        }

        if !self.glob_prefixes.is_empty() {
            let candidates: Vec<String> = self
                .glob_prefixes
                .iter()
                .map(|prefix| format!("{prefix}::{first}"))
                .collect();
            // Even a single glob is reported as ambiguous: a later glob (in
            // this file or a glob-re-exported module) could add another match.
            return Resolution::Ambiguous(candidates);
        }

        Resolution::Unresolved
    }

    /// Canonicalizes a rendered path when the result is unambiguous.
    ///
    /// Ambiguous and unresolved paths are returned unchanged so callers never
    /// act on a guess.
    pub fn canonicalize(&self, path: &str) -> String {
        match self.resolve(path) {
            Resolution::Resolved(canonical) => canonical,
            Resolution::Ambiguous(_) | Resolution::Unresolved => path.to_string(),
        }
    }

    fn collect_items(&mut self, items: &[syn::Item]) {
        for item in items {
            match item {
                syn::Item::Use(item_use) => {
                    let mut prefix = Vec::new();
                    self.collect_tree(&item_use.tree, &mut prefix);
                }
                syn::Item::Mod(item_mod) => {
                    if let Some((_, nested)) = &item_mod.content {
                        self.collect_items(nested);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_tree(&mut self, tree: &syn::UseTree, prefix: &mut Vec<String>) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.collect_tree(&path.tree, prefix);
                prefix.pop();
            }
            syn::UseTree::Name(name) => {
                if name.ident == "self" {
                    if let Some(alias) = prefix.last() {
                        if !prefix.is_empty() {
                            self.add_alias(alias.clone(), prefix.join("::"));
                        }
                    }
                } else {
                    let mut target = prefix.clone();
                    target.push(name.ident.to_string());
                    self.add_alias(name.ident.to_string(), target.join("::"));
                }
            }
            syn::UseTree::Rename(rename) => {
                if rename.ident == "self" {
                    if !prefix.is_empty() {
                        self.add_alias(rename.rename.to_string(), prefix.join("::"));
                    }
                } else {
                    let mut target = prefix.clone();
                    target.push(rename.ident.to_string());
                    self.add_alias(rename.rename.to_string(), target.join("::"));
                }
            }
            syn::UseTree::Glob(_) => {
                if !prefix.is_empty() {
                    self.glob_prefixes.insert(prefix.join("::"));
                }
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.collect_tree(item, prefix);
                }
            }
        }
    }

    fn add_alias(&mut self, alias: String, target: String) {
        self.aliases.entry(alias).or_default().insert(target);
    }
}

fn join(target: &str, rest: &[&str]) -> String {
    if rest.is_empty() {
        target.to_string()
    } else {
        format!("{target}::{}", rest.join("::"))
    }
}

/// Splits a rendered path into trimmed segments.
///
/// Token-stream rendering inserts spaces around `::` (`Token :: new`), so the
/// segments are trimmed. Paths containing generics or other syntax return
/// `None` and are left untouched.
fn normalized_segments(path: &str) -> Option<Vec<&str>> {
    let path = path.trim().trim_start_matches("::").trim();
    if path.is_empty() {
        return None;
    }
    let segments: Vec<&str> = path.split("::").map(str::trim).collect();
    if segments.iter().any(|segment| !is_ident(segment)) {
        return None;
    }
    Some(segments)
}

fn is_ident(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imports(source: &str) -> ImportMap {
        ImportMap::from_file(&syn::parse_file(source).expect("test source must parse"))
    }

    #[test]
    fn resolves_an_aliased_import() {
        let map = imports("use soroban_sdk::token::Client as Token;");
        assert_eq!(
            map.resolve("Token"),
            Resolution::Resolved("soroban_sdk::token::Client".to_string())
        );
    }

    #[test]
    fn appends_the_remaining_path_to_an_alias() {
        let map = imports("use crate::storage::State as Store;");
        assert_eq!(
            map.resolve("Store::get"),
            Resolution::Resolved("crate::storage::State::get".to_string())
        );
    }

    #[test]
    fn resolves_grouped_imports() {
        let map = imports("use soroban_sdk::{storage::Instance as Store, Env};");
        assert_eq!(
            map.resolve("Store"),
            Resolution::Resolved("soroban_sdk::storage::Instance".to_string())
        );
        assert_eq!(
            map.resolve("Env"),
            Resolution::Resolved("soroban_sdk::Env".to_string())
        );
    }

    #[test]
    fn resolves_self_imports() {
        let map = imports("use soroban_sdk::storage::{self, Instance};");
        assert_eq!(
            map.resolve("storage"),
            Resolution::Resolved("soroban_sdk::storage".to_string())
        );
        assert_eq!(
            map.resolve("Instance"),
            Resolution::Resolved("soroban_sdk::storage::Instance".to_string())
        );
    }

    #[test]
    fn reports_glob_imports_as_ambiguous() {
        let map = imports("use soroban_sdk::*;");
        assert_eq!(
            map.resolve("Env"),
            Resolution::Ambiguous(vec!["soroban_sdk::Env".to_string()])
        );
    }

    #[test]
    fn prefers_an_explicit_alias_over_a_glob() {
        let map = imports("use soroban_sdk::*;\nuse crate::token::Client as Token;");
        assert_eq!(
            map.resolve("Token"),
            Resolution::Resolved("crate::token::Client".to_string())
        );
    }

    #[test]
    fn duplicate_aliases_are_ambiguous() {
        let map = imports("use a::Client as Token;\nuse b::Client as Token;");
        assert_eq!(
            map.resolve("Token"),
            Resolution::Ambiguous(vec!["a::Client".to_string(), "b::Client".to_string()])
        );
    }

    #[test]
    fn qualified_paths_are_left_as_written() {
        let map = imports("use soroban_sdk::Env as E;");
        assert_eq!(
            map.resolve("soroban_sdk::Env"),
            Resolution::Resolved("soroban_sdk::Env".to_string())
        );
    }

    #[test]
    fn unknown_single_segments_are_unresolved() {
        let map = imports("use soroban_sdk::Env;");
        assert_eq!(map.resolve("env"), Resolution::Unresolved);
    }

    #[test]
    fn canonicalize_never_guesses_for_ambiguous_paths() {
        let map = imports("use soroban_sdk::*;");
        assert_eq!(map.canonicalize("Env"), "Env");
    }

    #[test]
    fn imports_inside_inline_modules_are_collected() {
        let map = imports("mod inner {\n    use soroban_sdk::token::Client as Token;\n}");
        assert_eq!(
            map.resolve("Token"),
            Resolution::Resolved("soroban_sdk::token::Client".to_string())
        );
    }

    #[test]
    fn rendered_path_spacing_is_normalized() {
        let map = imports("use crate::token::Client as Token;");
        assert_eq!(
            map.resolve("Token :: new"),
            Resolution::Resolved("crate::token::Client::new".to_string())
        );
    }

    #[test]
    fn glob_inside_a_group_is_recorded() {
        let map = imports("use soroban_sdk::{storage::*, Env};");
        assert_eq!(
            map.resolve("Instance"),
            Resolution::Ambiguous(vec!["soroban_sdk::storage::Instance".to_string()])
        );
    }
}
