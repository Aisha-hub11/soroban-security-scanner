# Soroban SDK compatibility matrix fixtures

Each directory is a minimal contract pinned to one supported `soroban-sdk`
release line. The CI `sdk-matrix` job scans every fixture and asserts that the
project is classified as a confirmed Soroban contract, that the pinned SDK
version is detected, and that the expected detectors still fire.

The fixtures intentionally share the same source. They exercise APIs that are
stable across the supported range (`#[contract]`, `#[contractimpl]`, persistent
storage, `symbol_short!`) while containing a known SS-001 and SS-008 finding.

| Fixture | Pinned SDK |
| ------- | ---------- |
| `soroban-sdk-20` | 20.5.0 (oldest supported line) |
| `soroban-sdk-21` | 21.7.7 (last v21 release) |
| `soroban-sdk-29` | 29.0.1 (current release) |

These are scanner fixtures, not production contracts. Do not add real secrets
or real account addresses.
