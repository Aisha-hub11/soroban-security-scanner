# Development Setup

## Prerequisites

- **Rust** (recent stable; CI builds and tests on stable).
- **Git**.
- Optional: [`just`](https://github.com/casey/just#installation) for the task
  runner described below.
- Optional: **Python 3.10+** for supporting tooling under `tools/`.

Check your toolchain:

```bash
rustc --version
cargo --version
```

### Windows note

Either the MSVC or the GNU target works. If `link.exe` is unavailable, install
the Visual Studio C++ Build Tools, or use the GNU toolchain with MinGW:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup component add rustfmt clippy --toolchain stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu
```

Using a directory override keeps the choice local to your checkout and out of
the committed configuration.

## Clone and build

```bash
git clone https://github.com/AyinkxLab/soroban-security-scanner
cd soroban-security-scanner
cargo build
```

## Run the CLI

```bash
cargo run -p soroban-scan-cli -- --help
cargo run -p soroban-scan-cli -- version
```

## Repository automation

`cargo xtask` provides offline maintenance commands (docs link checking, fixture
scaffolding, and backlog rendering). See
[Repository Automation](xtask.md).

```bash
cargo xtask --help
```

## Local quality gate

The repository ships a [`justfile`](../../justfile) so one command runs the same
gate as CI:

```bash
just check
```

The recipes are thin wrappers over cargo/xtask:

| Recipe | Runs |
| ------ | ---- |
| `just fmt` | `cargo fmt --all` |
| `just fmt-check` | `cargo fmt --all -- --check` |
| `just lint` | `cargo clippy --all-targets --all-features -- -D warnings` |
| `just test` | `cargo test --all --all-features` |
| `just docs` | `cargo xtask docs` |
| `just bench` | the in-process benchmark example |
| `just deny` | `cargo deny check` (requires `cargo-deny`) |
| `just check` | `fmt-check`, `lint`, `test`, and `docs` |
| `just ci` | `check` plus `deny` |

Without `just`, run the underlying commands directly:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --all-features
cargo xtask docs
```

## Formatting

```bash
cargo fmt --all
```

## Linting

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## Dependency audit

Install [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny):

```bash
cargo install cargo-deny --locked
cargo deny check
```

## Pre-commit (optional)

Install [`pre-commit`](https://pre-commit.com/) and enable the hooks:

```bash
pre-commit install
```

## Troubleshooting

- **`linker link.exe not found`** — you are on Windows without MSVC build tools;
  see the Windows note above.
- **Slow first build** — the first build downloads and compiles dependencies.
