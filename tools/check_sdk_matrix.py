#!/usr/bin/env python3
"""Validate a scanner JSON report from a pinned Soroban SDK fixture.

Used by the CI ``sdk-matrix`` job. The check is deliberately narrow: it proves
that the scanner classified the fixture as a Soroban contract, detected the
version pinned in its manifest, and still reports the expected detectors.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

DEFAULT_RULES = ("SS-001", "SS-008")


def validate(
    report: dict,
    sdk_version: str,
    required_rules: tuple[str, ...] = DEFAULT_RULES,
) -> list[str]:
    """Returns a list of human-readable problems; empty means success."""

    problems: list[str] = []
    project = report.get("project") or {}
    if project.get("kind") != "confirmed_soroban":
        problems.append(
            f"expected project.kind 'confirmed_soroban', got {project.get('kind')!r}"
        )

    versions = project.get("soroban_sdk_versions") or []
    if sdk_version not in versions:
        problems.append(
            f"expected soroban-sdk {sdk_version} in {versions!r}"
        )

    findings = report.get("findings") or []
    found_rules = {finding.get("rule_id") for finding in findings}
    for rule in required_rules:
        if rule not in found_rules:
            problems.append(f"expected a finding for {rule}")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, help="scanner JSON report")
    parser.add_argument(
        "--sdk",
        required=True,
        help="expected soroban-sdk version from the fixture manifest",
    )
    parser.add_argument(
        "--require-rule",
        action="append",
        dest="required_rules",
        default=None,
        help="rule id that must be reported (repeatable; default: SS-001 and SS-008)",
    )
    args = parser.parse_args()

    try:
        report = json.loads(args.report.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"error: cannot read {args.report}: {error}", file=sys.stderr)
        return 1

    required_rules = tuple(args.required_rules or DEFAULT_RULES)
    problems = validate(report, args.sdk, required_rules)
    if problems:
        print(f"sdk-matrix check failed for soroban-sdk {args.sdk}:")
        for problem in problems:
            print(f"  - {problem}")
        return 1

    print(
        f"OK: soroban-sdk {args.sdk} fixture classified as confirmed_soroban "
        f"with {', '.join(required_rules)} findings."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
