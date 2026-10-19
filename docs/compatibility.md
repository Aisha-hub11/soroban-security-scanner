# Compatibility

## Rust toolchain

- **Policy:** a recent stable Rust toolchain. CI builds and tests on stable for
  Linux, Windows, and macOS.
- **Verified with:** Rust 1.97.1.
- **MSRV:** not currently guaranteed. Historical toolchains could not be
  verified in the development environment, so no `rust-version` is declared. A
  pinned, tested MSRV is tracked as future work. Do not rely on a specific
  minimum until it is declared and CI-tested.

## Operating systems

| OS | Status |
| -- | ------ |
| Linux | Supported (CI tested) |
| macOS | Supported (CI tested) |
| Windows | Supported (CI tested; MSVC and GNU toolchains) |

## Soroban SDK

- **Detection:** version-agnostic. Any `soroban-sdk` (or `soroban-*` /
  `stellar-*`) dependency is recognized, and `#[contract]` / `#[contractimpl]`
  are used to confirm contract projects.
- **Detector targeting:** the built-in detectors model the modern Soroban SDK
  storage and authorization APIs (`env.storage().persistent()/instance()/temporary()`,
  `Address::require_auth`, generated `*Client` constructors). These correspond to
  SDK v20 and later.
- **Verified against fixtures pinned to** `soroban-sdk = "20.5.0"`,
  `"21.7.7"`, and `"29.0.1"` under `fixtures/sdk-matrix/`.

### SDK compatibility matrix

The CI `sdk-matrix` job scans the pinned fixture for each version and asserts
that:

- the fixture is classified as `confirmed_soroban`;
- the pinned version is detected in `project.soroban_sdk_versions`; and
- the SS-001 (missing authorization) and SS-008 (unchecked arithmetic) findings
  are still reported.

| SDK version | Fixture | Status |
| ----------- | ------- | ------ |
| 20.5.0 (oldest supported line) | `fixtures/sdk-matrix/soroban-sdk-20` | Verified |
| 21.7.7 (last v21 release) | `fixtures/sdk-matrix/soroban-sdk-21` | Verified |
| 29.0.1 (current release) | `fixtures/sdk-matrix/soroban-sdk-29` | Verified |

The matrix exercises scanner behavior, not contract compilation. The shared
fixture source deliberately sticks to APIs stable across these releases, so a
failure indicates detector or project-detection drift rather than an SDK build
problem. Run the same check locally after building the scanner:

```bash
soroban-scan scan fixtures/sdk-matrix/soroban-sdk-21 \
  --format json \
  --output sdk-21.json \
  --fail-on none
python tools/check_sdk_matrix.py sdk-21.json --sdk 21.7.7
```

## Machine-readable formats

| Format | Version | Stability |
| ------ | ------- | --------- |
| JSON report | tool version | Stable shape; additive changes only within a minor line. |
| SARIF | 2.1.0 | Stable. |
| Baseline file | `version: 1` | Versioned; unsupported versions are rejected. |
| Configuration | schema v0 | Unknown fields are rejected; additive changes are announced. |
| Rule ids | `SS-###` | Permanent and never reused. |

## Exit codes

Stable; see [CLI Reference](cli.md#exit-codes).
