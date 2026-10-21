#!/usr/bin/env python3
"""Extract a release's notes from CHANGELOG.md.

The publish workflow uses this script so the GitHub release body always comes
from the matching changelog section. It is read-only, offline, and fails hard
when the section is missing or empty -- a release must never be published with
notes that do not exist.

Usage:
    python3 tools/release_notes.py --tag v0.2.0 --output release-notes.md
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

#: ``## [0.2.0] - 2026-09-28`` or ``## [Unreleased]``.
HEADING_RE = re.compile(r"^## \[(?P<version>[^\]]+)\](?: - (?P<date>.+))?\s*$")
#: Any level-2 changelog heading ends the current section.
SECTION_RE = re.compile(r"^## (?=\[)")


class ReleaseNotesError(ValueError):
    """The changelog cannot supply notes for the requested tag."""


def normalize_tag(tag: str) -> str:
    """Return the changelog version for a ``vX.Y.Z`` tag."""
    value = (tag or "").strip()
    if value.startswith("v"):
        value = value[1:]
    if not value:
        raise ReleaseNotesError("a non-empty tag is required")
    return value


def extract_section(text: str, tag: str) -> str:
    """Return the changelog section body for ``tag``.

    Raises:
        ReleaseNotesError: When the section is missing, empty, or the tag is
            malformed.
    """
    version = normalize_tag(tag)
    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        match = HEADING_RE.match(line)
        if match and match.group("version").strip() == version:
            start = index + 1
            break
    if start is None:
        raise ReleaseNotesError(
            f"CHANGELOG.md has no section for version {version!r} (tag {tag!r})"
        )

    end = len(lines)
    for index in range(start, len(lines)):
        if SECTION_RE.match(lines[index]):
            end = index
            break

    body = "\n".join(lines[start:end]).strip()
    if not body:
        raise ReleaseNotesError(
            f"CHANGELOG.md section for version {version!r} is empty"
        )
    return body + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True, help="release tag, for example v0.2.0")
    parser.add_argument(
        "--changelog",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "CHANGELOG.md",
        help="path to CHANGELOG.md (default: repository root)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="write notes to this file instead of stdout",
    )
    args = parser.parse_args(argv)

    try:
        text = args.changelog.read_text(encoding="utf-8")
        notes = extract_section(text, args.tag)
    except (OSError, ReleaseNotesError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.output:
        args.output.write_text(notes, encoding="utf-8")
        print(f"wrote {len(notes.splitlines())} line(s) to {args.output}")
    else:
        sys.stdout.write(notes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
