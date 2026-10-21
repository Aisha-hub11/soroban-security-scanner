"""Tests for tools/release_notes.py (issue #91)."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

_SPEC = importlib.util.spec_from_file_location(
    "release_notes", ROOT / "tools" / "release_notes.py"
)
assert _SPEC and _SPEC.loader
release_notes = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(release_notes)

CHANGELOG = """# Changelog

## [Unreleased]

- Work in progress.

## [0.3.0] - 2026-10-01

### Added

- A new detector.

### Fixed

- A parser bug.

## [0.3.0-rc.1] - 2026-09-20

- Release candidate only.

## [0.2.0] - 2026-09-28

- Older release.
"""


class ExtractSectionTests(unittest.TestCase):
    def test_extracts_exact_section_and_stops_at_next_heading(self):
        notes = release_notes.extract_section(CHANGELOG, "v0.3.0")
        self.assertIn("### Added", notes)
        self.assertIn("- A new detector.", notes)
        self.assertIn("- A parser bug.", notes)
        self.assertNotIn("Release candidate", notes)
        self.assertNotIn("Older release", notes)

    def test_accepts_a_tag_without_the_v_prefix(self):
        notes = release_notes.extract_section(CHANGELOG, "0.2.0")
        self.assertEqual(notes.strip(), "- Older release.")

    def test_supports_pre_release_tags(self):
        notes = release_notes.extract_section(CHANGELOG, "v0.3.0-rc.1")
        self.assertIn("Release candidate only", notes)

    def test_does_not_match_the_unreleased_section(self):
        with self.assertRaises(release_notes.ReleaseNotesError):
            release_notes.extract_section(CHANGELOG, "v9.9.9")

    def test_missing_section_fails(self):
        with self.assertRaises(release_notes.ReleaseNotesError) as caught:
            release_notes.extract_section(CHANGELOG, "v1.0.0")
        self.assertIn("no section", str(caught.exception))

    def test_empty_section_fails(self):
        text = "# Changelog\n\n## [1.0.0] - 2026-10-01\n\n## [0.9.0]\n\n- old\n"
        with self.assertRaises(release_notes.ReleaseNotesError) as caught:
            release_notes.extract_section(text, "v1.0.0")
        self.assertIn("empty", str(caught.exception))

    def test_empty_tag_fails(self):
        with self.assertRaises(release_notes.ReleaseNotesError):
            release_notes.normalize_tag("  ")


class CommandLineTests(unittest.TestCase):
    def test_writes_the_section_to_the_output_file(self):
        with tempfile.TemporaryDirectory() as directory:
            changelog = Path(directory) / "CHANGELOG.md"
            output = Path(directory) / "notes.md"
            changelog.write_text(CHANGELOG, encoding="utf-8")

            code = release_notes.main(
                ["--tag", "v0.3.0", "--changelog", str(changelog), "--output", str(output)]
            )

            self.assertEqual(code, 0)
            self.assertIn("- A new detector.", output.read_text(encoding="utf-8"))

    def test_missing_section_returns_non_zero(self):
        with tempfile.TemporaryDirectory() as directory:
            changelog = Path(directory) / "CHANGELOG.md"
            changelog.write_text(CHANGELOG, encoding="utf-8")
            code = release_notes.main(
                ["--tag", "v9.9.9", "--changelog", str(changelog)]
            )
            self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
