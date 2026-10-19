# GitHub integration scripts

These scripts are **CI-only** helpers used by the workflows in
`.github/workflows/`. Unlike the offline tooling in `tools/`, they call the
GitHub CLI (`gh api`) and therefore need network access and a workflow token.

| Script | Purpose | Required token permission |
| ------ | ------- | ------------------------- |
| `pr_comment.py` | Create or update the single sticky pull-request comment. | `pull-requests: write` |
| `check_run.py` | Create a check run with inline annotations from a JSON report. | `checks: write` |

Both scripts never print or persist the token; `gh` reads it from `GH_TOKEN`.
They support `--dry-run` for local inspection. Run the tests with:

```bash
python -m unittest discover -s tests/github -v
```
