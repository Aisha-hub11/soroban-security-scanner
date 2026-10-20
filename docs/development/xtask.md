# Repository Automation (`xtask`)

Repository automation lives in the `xtask` workspace crate. It is a typed,
portable entry point for the maintenance tasks that used to be Python-only.
Every command is **offline** and **non-destructive**.

```bash
cargo xtask --help
```

```
Usage: xtask <COMMAND>

Commands:
  docs      Check relative links in every Markdown file
  fixtures  Scaffold a project fixture under fixtures/projects/
  backlog   Render the contributor backlog from tools/backlog/*.json
```

The alias is defined in `.cargo/config.toml`:

```toml
[alias]
xtask = "run --package xtask --"
```

## `cargo xtask docs`

Checks every `*.md` file for relative links that point at a missing file. It is
a Rust reimplementation of `tools/check_docs.py` and produces the same summary:

```bash
cargo xtask docs
# OK: checked 79 Markdown files, no broken relative links.
```

External URLs, `mailto:` links, anchors, and `<...>` targets are not fetched or
checked. `.git`, `target`, `node_modules`, and `vendor` are skipped.

Use `--root <dir>` to check another checkout.

## `cargo xtask fixtures <name>`

Scaffolds a project fixture under `fixtures/projects/<name>`:

```bash
cargo xtask fixtures nested-call
cargo xtask fixtures nested-call --force   # overwrite scaffold files only
```

The generated fixture is a minimal, parseable Rust crate (`Cargo.toml` and
`src/lib.rs`) intended as a starting point for a regression corpus. Fixture
names are restricted to letters, digits, `-`, and `_`; path traversal is
rejected. Existing fixtures are never overwritten without `--force`, and
`--force` only replaces the two scaffold files — unrelated files are left
intact.

## `cargo xtask backlog`

Renders the contributor backlog from `tools/backlog/*.json` into Markdown issue
bodies, mirroring the body format of `tools/create_backlog.py`:

```bash
cargo xtask backlog                  # writes target/backlog/NNN-<slug>.md
cargo xtask backlog --check          # validate JSON and duplicate titles only
cargo xtask backlog --output /tmp/backlog
```

`--check` parses every file, verifies required fields and bullet types, and
rejects duplicate titles. It does not write anything and does not contact
GitHub. Creating labels and issues still requires `gh`, so that step remains in
`tools/create_backlog.py` until xtask reaches parity.

## Relationship to the Python tools

The Python helpers under `tools/` remain supported and unchanged:

| Task | Python | xtask |
| ---- | ------ | ----- |
| Markdown link check | `python tools/check_docs.py` | `cargo xtask docs` |
| Fixture scaffolding | — | `cargo xtask fixtures <name>` |
| Render backlog bodies | `python tools/create_backlog.py` (creates issues) | `cargo xtask backlog` (offline render) |

xtask only replaces read-only and local-generation work. Anything that needs
credentials or the network stays opt-in in the Python tooling.
