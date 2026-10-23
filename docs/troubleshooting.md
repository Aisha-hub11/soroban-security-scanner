# Troubleshooting

## The scanner reports no findings on my Soroban project

- Confirm the project is recognized: the terminal header shows the project kind.
  If it says `generic_rust`, the scanner found no Soroban dependency or
  `#[contract]`/`#[contractimpl]`. Check that `Cargo.toml` declares
  `soroban-sdk` and that the contract macro is present.
- If the project is classified as `likely_soroban`, detectors that only apply to
  contract entry points will not run. This is intentional: the scanner does not
  guess.
- Check `--min-severity` / `--min-confidence` and any `disabled` rules.

## I disagree with a finding (false positive)

1. Read the rule's documentation: `soroban-scan explain SS-00X`.
2. Suppress it for the project if warranted:

   ```toml
   [rules]
   disabled = ["SS-006"]
   ```

3. If the rule is genuinely wrong, please open a bug report with a minimal
   example. False positives are treated as first-class bugs.

## Findings appear on lines I did not edit

Findings are tracked by **fingerprint**, not by line number. A fingerprint is
derived from the rule id, the scan-relative file path, and normalized evidence,
so it survives line shifts and edits elsewhere in the file. A baseline can
therefore suppress a finding that now appears on a different line, and a finding
may be reported on a line you did not change.

See [Baselines](baselines.md#fingerprints) for the exact fingerprint inputs and
how baseline matching works. Review a shifted finding by its evidence and rule
id rather than its line number.

## A file or directory is not scanned

- `target/`, `.git/`, `node_modules/`, and `vendor/` are skipped by default.
- Check `exclude` globs in configuration.
- Symlinks are not followed by default; enable `follow_symlinks` only if you
  trust the target.

## "unknown rule id" error

The id does not exist. List valid ids with `soroban-scan rules`. Ids are of the
form `SS-###`.

## Configuration is rejected

Unknown fields are errors. Run `soroban-scan config validate` for a message, and
compare against [Configuration](configuration.md).

## `linker link.exe not found` (Windows)

Install the Visual Studio C++ Build Tools, or switch to the GNU toolchain:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu
```

## Build fails with a toolchain error

Use a recent stable Rust toolchain. Update with `rustup update stable`. See
[Compatibility](compatibility.md) for the support policy.

## Exit code 1 in CI, but there are no "new" findings

Exit code `1` means a finding at or above `--fail-on` is present. Without a
baseline, that includes pre-existing findings. Add a baseline
(see [Baselines](baselines.md)) or set `--fail-on none`.

## SARIF upload works but no results appear on GitHub

- The upload step must have `security-events: write` permission. On private
  repositories it also needs `actions: read`.
- Fork pull requests cannot upload SARIF; guard the step with the fork check in
  [GitHub Integration](github-integration.md).
- Code scanning must be enabled for the repository (it is automatic for public
  repositories on first upload).
- For private or internal repositories, **GitHub Code Security** must be
  enabled. Without it the upload fails with "GitHub Code Security or GitHub
  Advanced Security must be enabled for this repository to use code scanning".
  See [Private repositories](github-integration.md#private-repositories).
- The repository or organization Actions policy must allow
  `github/codeql-action`, which provides the `upload-sarif` action.

## The scan is slow on a large repository

Parsing dominates. See [Benchmarks](development/benchmarks.md) and exclude large
generated or vendor directories.

## Still stuck?

Open a discussion or issue on the
[repository](https://github.com/AyinkxLab/soroban-security-scanner/issues). Do
not report security vulnerabilities publicly; see
[SECURITY.md](../SECURITY.md).
