# Doctor

`doctor` explains how a project was classified and which configuration is in
effect. It is read-only: it loads project metadata, parses sources, and prints a
diagnostic summary without scanning for findings.

```bash
soroban-scan doctor ./contracts
soroban-scan doctor ./contracts --format json
```

## What it reports

- Scan root (relative when possible) and discovered manifests.
- Project classification: `generic_rust`, `likely_soroban`, or
  `confirmed_soroban`.
- Classification evidence, with absolute scan-root paths removed.
- Number of source files discovered and loaded.
- Exclude globs, `exclude_tests`, and `follow_symlinks`.
- The effective configuration and the rule ids that will run.

## Options

| Option | Description |
| ------ | ----------- |
| `--format <terminal\|json>` | Output format (default `terminal`). |
| `--config <FILE>` | Use a specific configuration file. |
| `--no-config` | Ignore discovered configuration and use built-in defaults. |

## Security notes

Doctor output never includes environment variables, secrets, or absolute home
paths. Evidence text that contains the scan root is rewritten to a relative
label.

## Debugging a clean scan

A scan that reports no findings may be correct: the project may be classified as
generic Rust, a rule may be disabled, or an exclude may cover the source. Run
`doctor` to check the classification, exclusions, and enabled rules before
changing the scanner configuration.

```bash
soroban-scan doctor . --format json
```
