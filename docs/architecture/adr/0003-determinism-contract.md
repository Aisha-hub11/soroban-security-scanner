# ADR-0003: Determinism contract

- **Status:** Accepted
- **Date:** 2026-10-20
- **Deciders:** AyinkxLab maintainers

## Context

Determinism is a core product promise: the same repository, configuration, and
tool version must produce the same findings, in the same order, with the same
fingerprints and byte-identical machine-readable output. Downstream consumers
such as the Stellar Contract Observatory rely on this stability to diff reports
and track findings over time.

Until now the promise has lived mostly in code comments and individual tests.
That is easy to regress: a new rule can iterate a `HashMap`, a renderer can sort
with an unstable comparator, or a diagnostic can include a temp path. This ADR
records the contract so new contributors have one authoritative reference.

## Decision

### 1. Canonical ordering

Findings are sorted by this tuple, in ascending order:

1. file path, normalized to forward slashes and relative to the scan root when
   possible;
2. 1-based line number;
3. 1-based column number, with an unknown column treated as `0`;
4. rule id (byte order);
5. fingerprint (byte order), as the final tie-breaker.

Every output format renders findings in that order. Summary tables that group
findings (rule and category counts) sort by descending count and then by name or
id. Baselines are emitted sorted by fingerprint. Discovery sorts manifests and
source files, and rule metadata is emitted in a stable order.

Rules execute in ascending **rule id** order, independent of registration order
(issue #106). Diagnostics, per-rule timings, and future early-exit decisions
therefore match across registries containing the same rules in a different
order. The final finding sort remains the last line of defence.

### 2. Fingerprint stability

A fingerprint is the first 16 bytes of SHA-256 over:

```
rule_id "\0" normalized_file_path "\0" normalized_evidence
```

`normalized_file_path` uses forward slashes. `normalized_evidence` collapses all
runs of whitespace to a single space and trims the ends. Line and column numbers
are deliberately excluded so a finding survives unrelated line shifts, which is
required for baselines. Changing this recipe is a breaking report-contract
change and needs a superseding ADR.

### 3. Forbidden inputs

The scanning path must not read or depend on:

- **Wall-clock time.** No `SystemTime::now`, `Instant`, or date formatting in
  rule evaluation, sorting, fingerprints, or output. Timestamps in generated
  reports (if any) must come from an explicit caller-supplied value.
- **Randomness.** No `rand`, UUID generation, hash-map iteration order, or
  thread scheduling. Process ids and temporary directory names must never reach
  machine-readable output.
- **Filesystem order.** Directory entries are sorted before analysis. Symlink
  traversal is opt-in and still sorted.
- **Locale and environment.** No locale-dependent case folding or collation;
  comparisons use byte order. Environment variables may configure a scan but
  must not be serialized into reports.

### 4. Bounded, deterministic failures

Unreadable files, oversized inputs, parse failures, and ambiguous crate-root
associations are reported as diagnostics with stable codes. A diagnostic's code
and level are part of the contract; its human-readable message is not. Failures
must never introduce unordered or partially-read data into the output.

## Enforcement

The contract is enforced by tests, not convention:

- `crates/soroban-scan-core/tests/adversarial.rs` —
  `repeated_scans_are_byte_identical`.
- `crates/soroban-scan-core/tests/corpus.rs` —
  `corpus_scanning_is_deterministic`.
- `crates/soroban-scan-core/tests/project_detection.rs` —
  `metadata_loading_is_deterministic`.
- `crates/soroban-scan-core/src/engine.rs` — `scanning_is_deterministic`.
- `crates/soroban-scan-core/tests/ordering_contract.rs` — every output format
  receives the same finding order.
- `crates/soroban-scan-core/src/finding.rs` — fingerprint stability and
  whitespace insensitivity.

A pull request that changes ordering, fingerprint inputs, or any of the
forbidden inputs above must update these tests and this ADR in the same change.

## Consequences

- Reports are diffable in CI and across releases.
- Baselines survive refactors that only move code.
- Contributors must use ordered collections (`BTreeMap`/`Vec` plus an explicit
  sort) on the scanning path, even when a hash map would be more convenient.
- Adding a new output format requires routing it through the existing
  `report::render` dispatcher so the ordering contract applies automatically.

## Alternatives considered

- **Rely on documentation comments only.** Rejected: comments drift and do not
  fail CI.
- **Sort only JSON output.** Rejected: terminal, Markdown, SARIF, and future
  formats would diverge, breaking user expectations and tests.
- **Include line numbers in fingerprints.** Rejected: harmless edits would
  invalidate baselines and create noisy diffs.

## References

- [ADR-0001: Rust core with Python supporting tooling](0001-rust-core-python-tooling.md)
- [ADR-0005: Dependency justification policy](0005-dependency-justification-policy.md)
- [Architecture overview](../overview.md)
- [Testing guide](../../development/testing.md)
