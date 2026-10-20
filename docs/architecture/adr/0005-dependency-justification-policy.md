# ADR-0005: Dependency justification policy

- **Status:** Accepted
- **Date:** 2026-10-19
- **Deciders:** AyinkxLab maintainers

## Context

The scanner's trustworthiness depends on a small, auditable dependency surface.
Every dependency adds build time, license obligations, advisory exposure, and a
potential supply-chain path into a tool that runs on untrusted code. ADR-0001
already states that dependencies must earn their place; this ADR makes the
review rule explicit.

The repository enforces dependency hygiene with `cargo-deny` and `deny.toml`
(advisories, licenses, bans, and sources). That gate catches known-bad states,
but it cannot decide whether a new dependency is justified for this project.

## Decision

A new or changed dependency is accepted only when all of the following hold:

1. **Necessary.** The capability cannot be implemented safely with the standard
   library or an existing dependency without disproportionate complexity.
2. **Scoped.** The dependency is used for one clear purpose; broad frameworks are
   avoided when a focused crate exists.
3. **Maintained.** The crate has an active maintainer, recent releases, and a
   documented security contact or advisory history.
4. **Licensed.** Its license is already allowed in `deny.toml` or the exception
   is reviewed and recorded in the same pull request.
5. **Auditable.** The version is pinned in `Cargo.lock`, the source is the
   crates.io registry (or a reviewed git source), and no wildcard requirement is
   introduced.
6. **Justified in the pull request.** The description states what the dependency
   does, why an alternative is unsuitable, its transitive footprint, and any
   feature flags disabled to reduce it.
7. **Reviewed.** At least one maintainer reviews the dependency addition
   separately from the feature using it.

`cargo deny check` must pass in CI. A dependency change that needs a `deny.toml`
exception must add the exception and the justification in the same reviewable
change.

## Alternatives considered

- **Allow dependencies by default and rely on `cargo-deny`.** Rejected: the gate
  checks policy compliance, not necessity or scope.
- **Ban all new dependencies.** Rejected: some capabilities (parsing, TLS,
  serialization) are safer and cheaper to reuse than to reimplement.
- **Review only direct dependencies.** Rejected: transitive crates carry the same
  supply-chain and license risk.

## Consequences

- A pull request that adds a dependency may take longer to review; that is
  intentional.
- `cargo-deny` remains the automated backstop, while the ADR supplies the human
  decision rule.
- Dependency removals and feature-flag reductions are always welcome and do not
  need a separate ADR.

## References

- [ADR-0001: Rust core with Python supporting tooling](0001-rust-core-python-tooling.md)
- [`deny.toml`](../../../deny.toml)
- [cargo-deny documentation](https://embarkstudios.github.io/cargo-deny/)
