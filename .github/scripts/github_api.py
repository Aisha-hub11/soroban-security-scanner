#!/usr/bin/env python3
"""Small wrapper around ``gh api`` for the GitHub integration scripts.

The workflows pass a short-lived ``GITHUB_TOKEN`` through the environment. This
module never prints, stores, or forwards that token: authentication is handled
entirely by the ``gh`` CLI.
"""

from __future__ import annotations

import json
import subprocess
from typing import Any, Callable, Sequence


class GitHubApiError(RuntimeError):
    """Raised when ``gh api`` exits unsuccessfully."""


def run_gh(
    arguments: Sequence[str],
    payload: dict[str, Any] | None = None,
    *,
    timeout: int = 60,
) -> str:
    """Runs ``gh api`` with ``arguments`` and returns stdout.

    ``payload`` is sent as a JSON request body through stdin so comment bodies
    and annotation summaries never have to be escaped into command-line
    arguments.
    """

    command = ["gh", "api", *arguments]
    result = subprocess.run(
        command,
        input=json.dumps(payload) if payload is not None else None,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
    )
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip()
        raise GitHubApiError(f"gh api failed with exit code {result.returncode}: {message}")
    return result.stdout


def parse_json(text: str) -> Any:
    """Parses a ``gh api`` response, treating an empty response as ``None``."""

    text = text.strip()
    if not text:
        return None
    return json.loads(text)


def list_comments(repo: str, number: int, *, runner: Callable[..., str] = run_gh) -> list[dict[str, Any]]:
    """Returns every issue/PR comment, following GitHub pagination."""

    endpoint = f"repos/{repo}/issues/{number}/comments?per_page=100"
    pages = parse_json(runner(["--paginate", "--slurp", endpoint]))
    if pages is None:
        return []
    # ``--slurp`` wraps each page in an outer array. Older gh versions emit a
    # flat array when there is only one page, so tolerate both shapes.
    if pages and isinstance(pages[0], dict):
        return list(pages)
    comments: list[dict[str, Any]] = []
    for page in pages:
        comments.extend(page)
    return comments


def create_comment(
    repo: str,
    number: int,
    body: str,
    *,
    runner: Callable[..., str] = run_gh,
) -> dict[str, Any]:
    """Creates a comment and returns the API response."""

    endpoint = f"repos/{repo}/issues/{number}/comments"
    response = parse_json(
        runner(["--method", "POST", "--input", "-", endpoint], {"body": body})
    )
    if not isinstance(response, dict):
        raise GitHubApiError("unexpected response when creating a comment")
    return response


def update_comment(
    repo: str,
    comment_id: int,
    body: str,
    *,
    runner: Callable[..., str] = run_gh,
) -> dict[str, Any]:
    """Updates an existing comment and returns the API response."""

    endpoint = f"repos/{repo}/issues/comments/{comment_id}"
    response = parse_json(
        runner(["--method", "PATCH", "--input", "-", endpoint], {"body": body})
    )
    if not isinstance(response, dict):
        raise GitHubApiError("unexpected response when updating a comment")
    return response


def delete_comment(
    repo: str,
    comment_id: int,
    *,
    runner: Callable[..., str] = run_gh,
) -> None:
    """Deletes a comment. GitHub returns ``204 No Content`` on success."""

    endpoint = f"repos/{repo}/issues/comments/{comment_id}"
    runner(["--method", "DELETE", endpoint])


def create_check_run(
    repo: str,
    payload: dict[str, Any],
    *,
    runner: Callable[..., str] = run_gh,
) -> dict[str, Any]:
    """Creates a check run and returns the API response."""

    endpoint = f"repos/{repo}/check-runs"
    response = parse_json(
        runner(["--method", "POST", "--input", "-", endpoint], payload)
    )
    if not isinstance(response, dict):
        raise GitHubApiError("unexpected response when creating a check run")
    return response
