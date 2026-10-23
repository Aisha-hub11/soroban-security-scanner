# Repository task runner (#77).
#
# Use the default POSIX shell on Unix; PowerShell keeps the same recipes working
# on Windows, where `sh` is not guaranteed to be installed.

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
#
# Recipes are deliberately thin wrappers over cargo/xtask so CI and contributors
# run exactly the same commands. Install `just` from
# https://github.com/casey/just#installation; `cargo-deny` is only required for
# the `deny` recipe.

# Show the available recipes.
default:
    @just --list

# Format the workspace.
fmt:
    cargo fmt --all

# Verify formatting without writing changes.
fmt-check:
    cargo fmt --all -- --check

# Run clippy with the same warnings-as-errors gate as CI.
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run the full test suite.
test:
    cargo test --all --all-features

# Check relative links in every Markdown file.
docs:
    cargo xtask docs

# Run the dependency-free in-process benchmark.
bench:
    cargo run --release -p soroban-scan-core --example benchmark

# Audit dependencies against deny.toml (requires cargo-deny).
deny:
    cargo deny check

# Full local gate: format, lint, tests, and documentation links.
check: fmt-check lint test docs

# Everything in `check` plus the dependency audit.
ci: check deny
