"""Tests for the SDK matrix report checker."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[2] / "tools"
sys.path.insert(0, str(TOOLS))

import check_sdk_matrix  # noqa: E402


def report(*, kind: str = "confirmed_soroban", versions=None, rules=None) -> dict:
    return {
        "project": {
            "kind": kind,
            "soroban_sdk_versions": versions or ["21.7.7"],
        },
        "findings": [{"rule_id": rule} for rule in (rules or ["SS-001", "SS-008"])],
    }


class ValidateTests(unittest.TestCase):
    def test_accepts_a_matching_report(self):
        self.assertEqual(
            check_sdk_matrix.validate(report(), "21.7.7"),
            [],
        )

    def test_rejects_the_wrong_project_kind(self):
        problems = check_sdk_matrix.validate(report(kind="generic_rust"), "21.7.7")
        self.assertTrue(any("confirmed_soroban" in problem for problem in problems))

    def test_rejects_a_missing_sdk_version(self):
        problems = check_sdk_matrix.validate(report(), "29.0.1")
        self.assertTrue(any("29.0.1" in problem for problem in problems))

    def test_rejects_a_missing_required_rule(self):
        problems = check_sdk_matrix.validate(report(rules=["SS-001"]), "21.7.7")
        self.assertTrue(any("SS-008" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
