"""Tests for the check-run helper."""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[2] / ".github" / "scripts"
sys.path.insert(0, str(SCRIPTS))

import check_run  # noqa: E402


def finding(
    *,
    rule_id: str = "SS-001",
    severity: str = "high",
    line: int = 10,
    file: str = "src/lib.rs",
    title: str = "State-changing entry point without caller authorization",
) -> dict:
    return {
        "rule_id": rule_id,
        "title": title,
        "severity": severity,
        "evidence": "`persistent` storage mutation with no authorization",
        "remediation": "Call `require_auth()` before mutating state.",
        "location": {"file": file, "line": line},
    }


class ConclusionTests(unittest.TestCase):
    def test_high_finding_fails_the_default_threshold(self):
        self.assertEqual(
            check_run.conclusion_for([finding(severity="high")], "high"),
            "failure",
        )

    def test_critical_finding_fails_a_high_threshold(self):
        self.assertEqual(
            check_run.conclusion_for([finding(severity="critical")], "high"),
            "failure",
        )

    def test_medium_finding_passes_a_high_threshold(self):
        self.assertEqual(
            check_run.conclusion_for([finding(severity="medium")], "high"),
            "success",
        )

    def test_medium_finding_fails_a_medium_threshold(self):
        self.assertEqual(
            check_run.conclusion_for([finding(severity="medium")], "medium"),
            "failure",
        )

    def test_none_disables_the_gate(self):
        self.assertEqual(
            check_run.conclusion_for([finding(severity="critical")], "none"),
            "success",
        )

    def test_no_findings_passes(self):
        self.assertEqual(check_run.conclusion_for([], "high"), "success")


class PayloadTests(unittest.TestCase):
    def test_payload_has_completed_status_and_annotations(self):
        report = {"findings": [finding(line=23)]}
        payload = check_run.build_payload(
            report,
            name="Soroban Security Scan",
            head_sha="abc123",
            fail_on="high",
        )
        self.assertEqual(payload["status"], "completed")
        self.assertEqual(payload["conclusion"], "failure")
        self.assertEqual(payload["head_sha"], "abc123")
        self.assertEqual(payload["name"], "Soroban Security Scan")
        annotations = payload["output"]["annotations"]
        self.assertEqual(len(annotations), 1)
        self.assertEqual(annotations[0]["path"], "src/lib.rs")
        self.assertEqual(annotations[0]["start_line"], 23)
        self.assertEqual(annotations[0]["annotation_level"], "failure")
        self.assertIn("SS-001", annotations[0]["message"])
        self.assertIn("require_auth", annotations[0]["raw_details"])

    def test_annotations_are_capped_at_the_github_limit(self):
        findings = [finding(line=index + 1) for index in range(55)]
        payload = check_run.build_payload(
            {"findings": findings},
            name="Soroban Security Scan",
            head_sha="abc123",
            max_annotations=check_run.GITHUB_ANNOTATION_LIMIT,
        )
        annotations = payload["output"]["annotations"]
        self.assertEqual(len(annotations), check_run.GITHUB_ANNOTATION_LIMIT)
        self.assertIn("5 finding(s) were not annotated", payload["output"]["summary"])

    def test_annotation_levels_follow_severity(self):
        levels = {
            "critical": "failure",
            "high": "failure",
            "medium": "warning",
            "low": "notice",
            "info": "notice",
        }
        for severity, level in levels.items():
            with self.subTest(severity=severity):
                self.assertEqual(
                    check_run.annotation_for(finding(severity=severity))[
                        "annotation_level"
                    ],
                    level,
                )

    def test_windows_paths_are_normalized(self):
        annotation = check_run.annotation_for(finding(file="src\\contracts\\lib.rs"))
        self.assertEqual(annotation["path"], "src/contracts/lib.rs")

    def test_empty_report_is_a_success_with_no_annotations(self):
        payload = check_run.build_payload(
            {"findings": []},
            name="Soroban Security Scan",
            head_sha="abc123",
        )
        self.assertEqual(payload["conclusion"], "success")
        self.assertEqual(payload["output"]["annotations"], [])
        self.assertEqual(payload["output"]["title"], "No security findings")

    def test_invalid_threshold_is_rejected(self):
        with self.assertRaises(ValueError):
            check_run.build_payload(
                {"findings": []},
                name="Soroban Security Scan",
                head_sha="abc123",
                fail_on="urgent",
            )

    def test_annotation_limit_above_github_limit_is_rejected(self):
        with self.assertRaises(ValueError):
            check_run.build_payload(
                {"findings": []},
                name="Soroban Security Scan",
                head_sha="abc123",
                max_annotations=check_run.GITHUB_ANNOTATION_LIMIT + 1,
            )

    def test_report_must_contain_findings_array(self):
        with self.assertRaises(ValueError):
            check_run.build_payload(
                {},
                name="Soroban Security Scan",
                head_sha="abc123",
            )


class CreateCheckRunTests(unittest.TestCase):
    def test_posts_the_payload_through_gh_api(self):
        calls = []

        def fake_runner(arguments, payload=None):
            calls.append((list(arguments), payload))
            return json.dumps({"id": 123, "html_url": "https://example.test/check"})

        report = {"findings": [finding()]}
        payload = check_run.build_payload(
            report,
            name="Soroban Security Scan",
            head_sha="abc123",
        )
        created = check_run.create_check_run("owner/repo", payload, runner=fake_runner)
        self.assertEqual(created["id"], 123)
        self.assertIn("--method", calls[0][0])
        self.assertEqual(calls[0][1], payload)


if __name__ == "__main__":
    unittest.main()
