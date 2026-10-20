# ADR-0002: Severity and confidence methodology

- **Status:** Accepted
- **Date:** 2026-10-20
- **Deciders:** AyinkxLab maintainers

## Context

Every finding carries two independent judgements:

- **severity** — how much harm follows if the issue is real;
- **confidence** — how strongly the evidence proves the issue is real.

Both are part of the report contract and drive `--min-severity`,
`--min-confidence`, the default `--fail-on high` gate, and the way users triage
results. The definitions currently live only in
[`docs/rules/README.md`](../../rules/README.md). Without a recorded decision,
future rules can inflate severity or confidence to make a finding look more
important, which erodes trust and creates alert fatigue.

The scanner is static and evidence-based. It parses source and reasons about
patterns; it does not execute contracts or observe runtime state. Confidence
must therefore describe the *static* evidence, not the likelihood that a
deployed contract will be exploited.

## Decision

### 1. Severity measures impact

Severity answers: **if this is real, what is the worst plausible impact?**

| Severity | Meaning |
| -------- | ------- |
| `critical` | Direct loss or theft of funds, or total loss of access control. |
| `high` | Serious security impact under plausible conditions. |
| `medium` | Security weakness that contributes to an exploit or degrades safety. |
| `low` | Minor weakness or hardening opportunity. |
| `info` | Informational; no direct security impact. |

Severity is a property of the **class of issue**, not of a particular fixture.
A rule declares one default severity; users may raise or lower the effective
severity through configuration, but the rule metadata stays honest.

### 2. Confidence measures evidence strength

Confidence answers: **how certain is the detector that this finding is real,
given only the evidence it can observe statically?**

| Confidence | Meaning |
| ---------- | ------- |
| `high` | Direct, unambiguous evidence in the source. |
| `medium` | Strong evidence, but assumptions or context are required. |
| `low` | Heuristic; may be a false positive; requires human review. |

`high` is reserved for evidence that does not depend on interprocedural
reasoning, configuration, or runtime behaviour the scanner cannot see.
`medium` is the normal ceiling for patterns that require reasonable but
unproven assumptions. `low` is appropriate for heuristics and broad matches.

### 3. Severity and confidence are independent

A high-impact issue detected heuristically is `severity: high,
confidence: low` — never `confidence: high`. Conversely, a certain observation
with little security impact is `severity: info, confidence: high`.

Confidence never raises severity. Severity never raises confidence. A rule may
not use “this would be critical if true” as a reason to claim high confidence.

### 4. The critical + high combination is forbidden for heuristics

Every rule is a static heuristic. A rule may not claim both maximum impact and
maximum certainty. The metadata validator in
[`crates/soroban-scan-core/src/rules/validation.rs`](../../../crates/soroban-scan-core/src/rules/validation.rs)
rejects `severity: critical, confidence: high` with the message:

> `critical severity with high confidence is not allowed for heuristics`

This is enforced for both unit-tested rules and repository-level rule-quality
tests. A genuinely critical issue should be reported as `critical` + `medium`
(or `high` + `high` when the evidence truly is unambiguous).

### 5. Gating follows severity, not confidence

The default `--fail-on high` gate fails on `high` and `critical` findings.
`--min-confidence` is a separate filter for noise control. CI should not lower
the failure threshold merely because a finding has `low` confidence; it should
review the evidence and suppress or baseline the finding explicitly.

## Rationale

- Users act on severity first and use confidence to decide how much manual
  verification is needed.
- Keeping the axes independent lets the scanner report a serious risk without
  pretending the static evidence is conclusive.
- The critical+high ban removes the most misleading metadata combination while
  still allowing truly critical findings to be reported at medium confidence.
- Conservative confidence is a security feature: understating certainty invites
  review; overstating it invites blind trust.

## Alternatives considered

- **Single “priority” score.** Rejected: it hides why a finding ranks where it
  does and makes tuning opaque.
- **Allow critical + high for “obvious” patterns.** Rejected: every rule is a
  heuristic, and the exception would be applied inconsistently.
- **Derive confidence from severity.** Rejected: impact and evidence are
  independent; coupling them defeats the purpose of two axes.

## Consequences

- Rule authors must justify severity by impact and confidence by evidence.
- Reviewers can reject metadata that overstates certainty without debating the
  detector logic.
- Reports remain stable and comparable across rules and releases.
- The definitions in `docs/rules/README.md` and this ADR must change together.

## Enforcement

- `RuleMetadata::validate` / `validate_metadata` rejects critical+high.
- `crates/soroban-scan-core/tests/rule_quality.rs` validates every built-in
  rule’s metadata and documentation.
- `docs/rules/README.md` is the user-facing definition; this ADR records why it
  exists and how to change it.

## References

- [Rules: severity, confidence, and detectors](../../rules/README.md)
- [ADR-0001: Rust core with Python supporting tooling](0001-rust-core-python-tooling.md)
- [ADR-0003: Determinism contract](0003-determinism-contract.md)
