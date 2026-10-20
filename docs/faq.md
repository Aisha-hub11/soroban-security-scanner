# Frequently Asked Questions

## 1. Does the scanner execute or compile the code it analyzes?

No. It parses Rust source and evaluates deterministic rules. It never builds,
runs, deploys, or mutates scanned code. See the
[Security Model](security/security-model.md).

## 2. Why did I get a finding that looks like a false positive?

Most detectors are syntactic and deliberately low-confidence when they cannot
prove impact. Review the evidence and the rule's limitations in the
[Rules](rules/README.md) reference. Use an inline suppression when the pattern is
known to be safe (see [Suppressions](suppressions.md)).

## 3. Does a finding fail my build automatically?

Only findings at or above `--fail-on` fail the build. The default is `high`.
Medium, low, and info findings are reported but do not fail the run. See the
[CLI Reference](cli.md).

## 4. How do I fail only on new findings?

Write a baseline once with `--write-baseline`, then scan with `--baseline` (and
optionally `--new-only`). The gate then applies to new findings only. See
[Baselines](baselines.md).

## 5. Can I suppress a single finding?

Yes. Add a comment such as
`// soroban-scan: ignore SS-001 -- reviewed by security`. Suppressed findings stay
visible in reports but are excluded from the gate. See
[Suppressions](suppressions.md).

## 6. Which Soroban SDK versions are supported?

The scanner analyzes source syntax, so it is not tightly coupled to one SDK
version. Compatibility fixtures cover pinned SDK releases; see
[Compatibility](compatibility.md).

## 7. Does it support workspaces and multiple crates?

Yes. Discovery walks the scan root and parses Rust sources across workspace
members. You can also pass several paths or a file list. See the
[CLI Reference](cli.md).

## 8. How do I run the scanner in CI?

Use the GitHub Action or a direct CLI call with SARIF, JSON, or Markdown output.
Copy-paste examples are in [CI Recipes](ci-recipes.md) and
[GitHub Integration](github-integration.md).

## 9. Is any data sent over the network?

No, not by default. The optional AI assistance layer is off by default and is
never authoritative. See [Optional AI Assistance](ai.md).

## 10. What do severity and confidence mean?

Severity describes potential impact; confidence describes how certain the
detector is from static evidence. They are independent. See
[Rules](rules/README.md) and
[Analysis Methodology](analysis-methodology.md).

## 11. How do I regenerate the rule table in the docs?

Run `soroban-scan rules --format markdown`. The output matches the table in the
[Rules](rules/README.md) reference.

## 12. Where do I report a missed detection or a false positive?

Use the security regression issue template and include a minimal snippet, the
expected and actual behavior, and the Soroban SDK version. Do not report
undisclosed vulnerabilities publicly; follow the
[security policy](../SECURITY.md).
