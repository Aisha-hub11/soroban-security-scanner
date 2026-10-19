"""Tests for the sticky pull-request comment helper."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[2] / ".github" / "scripts"
sys.path.insert(0, str(SCRIPTS))

import pr_comment  # noqa: E402


class FakeGh:
    """Minimal in-memory stand-in for the ``gh api`` command."""

    def __init__(self, comments=None):
        self.comments = list(comments or [])
        self.calls = []
        self.next_id = 100

    def __call__(self, arguments, payload=None):
        self.calls.append((list(arguments), payload))
        method = "GET"
        if "--method" in arguments:
            method = arguments[arguments.index("--method") + 1]

        if method == "GET":
            return json.dumps([self.comments])
        if method == "POST":
            comment = {
                "id": self.next_id,
                "body": payload["body"],
            }
            self.next_id += 1
            self.comments.append(comment)
            return json.dumps(comment)
        if method == "PATCH":
            comment_id = int(arguments[-1].rsplit("/", 1)[-1])
            for comment in self.comments:
                if comment["id"] == comment_id:
                    comment["body"] = payload["body"]
                    return json.dumps(comment)
            raise AssertionError(f"comment {comment_id} not found")
        if method == "DELETE":
            comment_id = int(arguments[-1].rsplit("/", 1)[-1])
            self.comments = [c for c in self.comments if c["id"] != comment_id]
            return ""
        raise AssertionError(f"unexpected method {method}")


class NormalizeBodyTests(unittest.TestCase):
    def test_adds_exactly_one_marker(self):
        body = pr_comment.normalize_body("### Scan\n\n1 finding\n")
        self.assertTrue(body.startswith(pr_comment.MARKER + "\n"))
        self.assertEqual(body.count(pr_comment.MARKER), 1)

    def test_existing_marker_is_not_duplicated(self):
        body = pr_comment.normalize_body(
            f"{pr_comment.MARKER}\n### Scan\n{pr_comment.MARKER}\n"
        )
        self.assertEqual(body.count(pr_comment.MARKER), 1)


class StickyCommentTests(unittest.TestCase):
    def test_creates_when_no_marked_comment_exists(self):
        fake = FakeGh(
            comments=[
                {"id": 1, "body": "unrelated review comment"},
                {"id": 2, "body": "another comment"},
            ]
        )
        summary = pr_comment.upsert_sticky_comment(
            "owner/repo", 36, "### Scan\n\n2 findings", runner=fake
        )
        self.assertEqual(summary["action"], "created")
        self.assertEqual(summary["duplicates_deleted"], 0)
        self.assertEqual(len(fake.comments), 3)
        self.assertIn(pr_comment.MARKER, fake.comments[-1]["body"])
        self.assertEqual(
            [call[0][0] for call in fake.calls], ["--paginate", "--method"]
        )

    def test_updates_the_existing_marked_comment(self):
        fake = FakeGh(
            comments=[
                {
                    "id": 7,
                    "body": f"{pr_comment.MARKER}\nold summary",
                }
            ]
        )
        summary = pr_comment.upsert_sticky_comment(
            "owner/repo", 36, "### Scan\n\nnew summary", runner=fake
        )
        self.assertEqual(summary["action"], "updated")
        self.assertEqual(summary["comment_id"], 7)
        self.assertEqual(len(fake.comments), 1)
        self.assertIn("new summary", fake.comments[0]["body"])

    def test_deletes_duplicate_marked_comments(self):
        fake = FakeGh(
            comments=[
                {"id": 7, "body": f"{pr_comment.MARKER}\nfirst"},
                {"id": 8, "body": f"{pr_comment.MARKER}\nsecond"},
                {"id": 9, "body": "not the scanner"},
            ]
        )
        summary = pr_comment.upsert_sticky_comment(
            "owner/repo", 36, "### Scan\n\nlatest", runner=fake
        )
        self.assertEqual(summary["action"], "updated")
        self.assertEqual(summary["duplicates_deleted"], 1)
        self.assertEqual([comment["id"] for comment in fake.comments], [7, 9])
        self.assertIn("latest", fake.comments[0]["body"])

    def test_dry_run_reads_but_does_not_write(self):
        fake = FakeGh(
            comments=[{"id": 7, "body": f"{pr_comment.MARKER}\nold"}]
        )
        summary = pr_comment.upsert_sticky_comment(
            "owner/repo", 36, "new", dry_run=True, runner=fake
        )
        self.assertEqual(summary["action"], "update")
        self.assertEqual(len(fake.calls), 1)
        self.assertNotIn("--method", fake.calls[0][0])
        self.assertIn("old", fake.comments[0]["body"])


class EventNumberTests(unittest.TestCase):
    def test_reads_pull_request_number_from_event(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "event.json"
            path.write_text(json.dumps({"pull_request": {"number": 42}}))
            self.assertEqual(pr_comment.default_pr_number(str(path)), 42)

    def test_returns_none_for_a_missing_event(self):
        self.assertIsNone(pr_comment.default_pr_number("does-not-exist.json"))


if __name__ == "__main__":
    unittest.main()
