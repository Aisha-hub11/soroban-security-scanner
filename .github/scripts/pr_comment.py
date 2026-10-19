#!/usr/bin/env python3
"""Create or update the single sticky scanner comment on a pull request.

GitHub does not provide "sticky" comments, so the workflow maintains one by
looking for a hidden HTML marker in existing comments. The first marked comment
is updated in place; any duplicate marked comments left by older runs are
deleted. Only when no marked comment exists is a new one created.

This script is intentionally CI-only: it calls ``gh api`` and therefore needs
network access and a token with ``pull-requests: write`` permission.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any, Callable, Sequence

from github_api import (
    create_comment,
    delete_comment,
    list_comments,
    run_gh,
    update_comment,
)

MARKER = "<!-- soroban-scan:sticky-comment -->"


def normalize_body(body: str, marker: str = MARKER) -> str:
    """Returns ``body`` with exactly one marker at the start."""

    cleaned = "\n".join(line for line in body.splitlines() if line.strip() != marker)
    return f"{marker}\n{cleaned.rstrip()}\n"


def find_marked_comments(
    comments: Sequence[dict[str, Any]], marker: str = MARKER
) -> list[dict[str, Any]]:
    """Returns comments containing the marker, oldest first."""

    return [comment for comment in comments if marker in (comment.get("body") or "")]


def upsert_sticky_comment(
    repo: str,
    number: int,
    body: str,
    *,
    marker: str = MARKER,
    dry_run: bool = False,
    runner: Callable[..., str] = run_gh,
) -> dict[str, Any]:
    """Creates or updates the sticky comment and removes duplicates.

    Returns a small, JSON-serializable summary suitable for workflow logs.
    """

    body = normalize_body(body, marker)
    comments = list_comments(repo, number, runner=runner)
    marked = find_marked_comments(comments, marker)

    if not marked:
        if dry_run:
            return {"action": "create", "comment_id": None, "duplicates_deleted": 0}
        created = create_comment(repo, number, body, runner=runner)
        return {
            "action": "created",
            "comment_id": created.get("id"),
            "duplicates_deleted": 0,
        }

    primary, *duplicates = marked
    primary_id = primary.get("id")
    if primary_id is None:
        raise RuntimeError("the marked pull request comment has no id")

    if dry_run:
        return {
            "action": "update",
            "comment_id": primary_id,
            "duplicates_deleted": len(duplicates),
        }

    if primary.get("body") != body:
        update_comment(repo, primary_id, body, runner=runner)

    for duplicate in duplicates:
        duplicate_id = duplicate.get("id")
        if duplicate_id is not None:
            delete_comment(repo, duplicate_id, runner=runner)

    return {
        "action": "updated",
        "comment_id": primary_id,
        "duplicates_deleted": len(duplicates),
    }


def default_pr_number(event_path: str | None = None) -> int | None:
    """Reads the pull request number from the workflow event payload."""

    event_path = event_path or os.environ.get("GITHUB_EVENT_PATH")
    if not event_path:
        return None
    try:
        event = json.loads(Path(event_path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    number = (event.get("pull_request") or {}).get("number") or event.get("number")
    return int(number) if number is not None else None


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        default=os.environ.get("GITHUB_REPOSITORY"),
        help="repository in owner/name form (default: GITHUB_REPOSITORY)",
    )
    parser.add_argument(
        "--pr",
        type=int,
        default=default_pr_number(),
        help="pull request number (default: read from GITHUB_EVENT_PATH)",
    )
    parser.add_argument(
        "--body-file",
        type=Path,
        required=True,
        help="Markdown file containing the comment body; the marker is added",
    )
    parser.add_argument(
        "--marker",
        default=MARKER,
        help="hidden HTML marker that identifies the sticky comment",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="list comments and report the intended action without writing",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if not args.repo:
        parser.error("--repo is required (or set GITHUB_REPOSITORY)")
    if args.pr is None:
        parser.error("--pr is required (or provide GITHUB_EVENT_PATH)")
    try:
        body = args.body_file.read_text(encoding="utf-8")
    except OSError as error:
        parser.error(f"cannot read {args.body_file}: {error}")

    try:
        summary = upsert_sticky_comment(
            args.repo,
            args.pr,
            body,
            marker=args.marker,
            dry_run=args.dry_run,
        )
    except Exception as error:  # noqa: BLE001 - report the API error to the workflow
        print(f"error: {error}", file=sys.stderr)
        return 1

    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
