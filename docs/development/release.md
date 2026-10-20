# Release Process

## Versioning

This project follows [Semantic Versioning](https://semver.org/). While the
version is `0.x`, minor releases may include breaking changes; they are always
documented in [`CHANGELOG.md`](../../CHANGELOG.md).

## Before releasing

Run the full gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
cargo run --release -p soroban-scan-core --example benchmark
```

CI must be green, including `cargo-deny` (advisories, licenses, bans, sources).

## Public API checks

The `semver` CI job runs
[`cargo-semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks) for
`soroban-scan-core` on every pull request. It compares the branch against the
latest `v*` tag and skips cleanly when no tag exists yet.

While the workspace version is `0.x`, the job passes `release-type: minor` so
pre-1.0 minor releases may contain breaking changes. Revisit that setting when
1.0 ships. The stable surface is documented in
[`docs/api-stability.md`](../api-stability.md).

## Release checklist

1. Ensure `main` is green in CI.
2. Update `CHANGELOG.md`: move `Unreleased` entries under the new version and
   add the date.
3. Bump `version` in the workspace `Cargo.toml` (`[workspace.package]`).
4. Update `docs/PROJECT_STATUS.md`.
5. Commit with `chore(release): vX.Y.Z`.
6. Create an annotated, signed tag:

   ```bash
   git tag -a vX.Y.Z -m "Soroban Security Scanner vX.Y.Z"
   git push origin main --follow-tags
   ```

7. Create the GitHub release:

   ```bash
   gh release create vX.Y.Z --title "vX.Y.Z" --notes-file CHANGELOG.md
   ```

8. Verify the release and tag on GitHub.

## Artifacts

Source is the primary artifact. Prebuilt binaries for Linux, macOS, and Windows
are attached to releases once a cross-compilation workflow is added (tracked as
future work).

## Package publishing

Tag releases publish both crates in lockstep through
`.github/workflows/publish.yml`. The workflow:

1. requires a `v*` tag and verifies it matches `[workspace.package].version`;
2. runs `cargo publish --dry-run` for `soroban-scan-core`;
3. publishes `soroban-scan-core`, waits for the crates.io index to serve that
   version, then runs the CLI dry-run and publishes `soroban-scan-cli`.

The token is a **least-privilege crates.io token** scoped to the two crate names
and stored as the `CARGO_REGISTRY_TOKEN` secret in the protected `crates-io`
environment. Tag pushes publish automatically; `workflow_dispatch` defaults to a
dry run and only publishes when run against a tag with `dry_run` disabled.
`cargo publish --dry-run` also runs on every pull request in the `package` CI
job.

Versions must stay in lockstep: both crates inherit `[workspace.package].version`
and the CLI depends on the exact same core version. Bump the workspace version
once, never the crates independently.

## Post-release

- Announce in the repository discussions.
- Open a fresh `Unreleased` section in the changelog.
