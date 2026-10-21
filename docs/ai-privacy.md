# AI assistance privacy and data flow

This document describes exactly what data can leave the machine when assistance
is enabled, what is excluded, where copies are kept on disk, and how to keep the
scanner fully offline.

The short version: **the default build ships no provider, opens no socket, and
cannot be made to**. The only implemented provider is the offline, rule-based
`rule-guidance` provider. A provider that does reach the network can only
obtain *finding context* — never whole files, environment variables, or
credentials — because the `Assistant` trait is the only door it has.

## What ships today

| | |
| --- | --- |
| Network provider | None. `ProviderRegistry::builtin()` registers only `rule-guidance`, which is offline and deterministic (`crates/soroban-scan-core/src/ai/mod.rs`). |
| `ai::assistant()` | Returns the offline `RuleGuidance` provider when `[ai] enabled = false`. When assistance is enabled it **fails closed**: `AiError::NotConfigured` when no `provider` is set, `AiError::ProviderUnavailable(name)` otherwise. Its return type is `Result<RuleGuidance, AiError>`, so no build can hand back a network-backed provider today. |
| Network transport | `ai::http` ships an audited, hardened GET client behind the non-default `ai-http` feature. It is a building block for a future provider; no shipped provider calls it. |

Because no provider is bundled, the "remote provider" path described below is a
**contract**, not a description of current traffic. It states what a provider
implementation will and will not be able to reach. Treat it as the privacy
promise a provider must keep, and check a provider against it before enabling
it.

## When a request can happen

A provider request is possible only when **all** of the following hold:

1. the user passes `--guidance`, so an assistant is constructed at all;
2. the configuration enables assistance (`[ai] enabled = true`);
3. a provider is compiled in and registered under the configured `provider` name;
4. that provider is willing to make a request, and the destination is allowed
   by the network policy;
5. at least one finding exists and the per-scan request budget
   (`max_requests`) has not been exhausted.

Today condition 3 is unsatisfiable in every build, because no network provider
is registered. If any condition fails, the scanner uses offline rule-based
guidance, prints a diagnostic, or exits without assistance — it never silently
falls back to a network call.

## Exact payload shape

A provider receives data through exactly two trait methods
(`crates/soroban-scan-core/src/ai/mod.rs`):

```rust
pub trait Assistant {
    fn name(&self) -> &str;
    fn explain_finding(&self, finding: &Finding, rule: Option<&RuleMetadata>) -> Explanation;
    fn summarize(&self, findings: &[Finding]) -> Explanation;
}
```

There is no request envelope, no JSON body, and no other parameter. The
contract is the argument list above.

### `explain_finding` — one finding

The provider receives one `Finding` and, when the rule is known, that rule's
`RuleMetadata`. The `Finding` fields, exactly as defined in
`crates/soroban-scan-core/src/finding.rs`:

| Field | Type | Content |
| ----- | ---- | ------- |
| `rule_id` | `String` | Stable rule id, for example `SS-001`. |
| `title` | `String` | Short rule title. |
| `description` | `String` | What the rule means. |
| `severity` | `Severity` | `critical`, `high`, `medium`, `low`, `info`. |
| `confidence` | `Confidence` | `high`, `medium`, `low`. |
| `category` | `Category` | Kebab-case rule category, for example `authorization`. |
| `location` | `SourceLocation` | See below. |
| `evidence` | `String` | The source snippet the rule matched. **The only source text in the contract.** |
| `remediation` | `String` | The rule's documented fix guidance. |
| `references` | `Vec<String>` | Public rule references. |
| `fingerprint` | `String` | Truncated SHA-256 digest (128-bit, 32 hex characters) over rule id, file path, and evidence. Used for baselines and as cache-key input. |

`location` is a `SourceLocation`:

| Field | Type | Content |
| ----- | ---- | ------- |
| `file` | `PathBuf` | Path **relative to the scan root**, serialized with forward slashes. |
| `line` | `usize` | 1-based line number. |
| `column` | `Option<usize>` | 1-based column, when known. |
| `end_line` | `Option<usize>` | 1-based end line, when known. |
| `end_column` | `Option<usize>` | 1-based end column, when known. |

The engine rejects absolute paths and `..` segments when it records a finding,
so `location.file` is root-relative. Note that the offline guidance text
renders the path through `display_label()`, which uses the platform separator;
only the serialized form is guaranteed to use forward slashes.

`RuleMetadata` adds the rule's own catalogue entries (id, title, description,
default severity and confidence, category, remediation, references). It is
documentation, not source code.

### `summarize` — the whole finding set

`summarize` receives `&[Finding]` — **every finding in the scan**, not a
filtered subset. The trait gives a provider no way to narrow that slice, so a
provider must not assume it sees only "interesting" findings. A provider that
summarizes locally is fine; a provider that forwards the slice to a remote
model forwards all of it.

## Data that is never available to a provider

Because the trait accepts only `Finding` and `RuleMetadata`, the following are
structurally out of reach:

- Whole source files, file bodies, or repository contents.
- Source text outside the rule's `evidence` snippet.
- Environment variables, API keys, tokens, credentials, or private keys.
- Absolute paths, home directories, usernames, hostnames, or machine names.
- Conversation history, prompts, model outputs from other tools, or telemetry.
- Secrets discovered by the scanner — assistance never receives secret values.

Two caveats that matter in practice:

- **`evidence` is source text.** It can contain proprietary code, because it is
  the snippet the rule matched. Review this with your organization before
  enabling a remote provider on proprietary code. Prefer a local provider or
  `--guidance` when that is not acceptable.
- **The "never" list is a contract, not a sandbox.** The scanner cannot stop a
  provider implementation from embedding extra text in its own output.
  `validate_explanation` rejects assistance that mentions an `SS-###` id that
  is not in the finding set, but it does not constrain the rest of the text.
  Review a provider before trusting it.

## Retention and on-disk copies

- **Scanner side:** the scanner does not persist request payloads. The
  assistance cache stores only the rendered explanation, keyed by a SHA-256
  digest of fingerprint + provider + model. Cache keys and filenames contain no
  evidence or source text, and the cache is bounded (256 entries, LRU).
- **The cached value does contain source text.** The rendered offline
  explanation embeds the `evidence` snippet and the file path, and it is
  written to disk at `.scanner-cache/ai/<sha256>.json`. Treat that directory as
  sensitive: it is excluded from discovery, but it is a plain file on disk.
  Summaries are not cached.
- **stderr:** `--guidance` prints the rendered explanations to stderr, and that
  text contains the evidence snippet and the file path. Redirect it to a file
  only if that file is as sensitive as the source itself.
- **Logs:** `--log-level` emits structured JSON lines that carry counts and
  diagnostic codes only — never request or response bodies, source snippets,
  secrets, or home paths.
- **Redaction:** every explanation passes through `ai::redact` at construction
  and again at render time. It removes common token formats (GitHub, AWS,
  OpenAI, Slack, Google, JWT, `Bearer`), secret-like assignments, PEM private
  key blocks, and absolute home directories. Redaction applies to *rendered
  output*, not to what the provider receives.
- **Provider side:** retention, training use, and subprocessors are controlled
  by the provider and your account. Read the provider's data-processing terms
  before enabling it. The scanner cannot delete data the provider has retained.
- **Network metadata:** the provider sees the destination, timing, request size,
  and whatever metadata its API requires. The scanner does not add user
  identity, telemetry, or tracking fields.

## Network policy

The optional transport in `crates/soroban-scan-core/src/ai/http.rs` enforces:

- HTTPS only, unless the policy explicitly opts in to plain HTTP;
- rejection of URLs with embedded credentials;
- an explicit destination allowlist — an empty or wildcard-only allowlist is
  rejected when the policy is built;
- connect and read timeouts, plus a hard response-size limit;
- manual redirect handling with a bounded redirect budget, where every redirect
  target is re-validated against the allowlist and the address rules;
- rejection of loopback, private, link-local, carrier-grade NAT,
  documentation, multicast, unspecified, and cloud-metadata addresses,
  including the usual metadata host names;
- fail-closed behavior on malformed responses and non-2xx statuses.

Two limits worth knowing:

- The client sends **no authentication headers of its own** — it is a hardened
  GET client with no request body. A provider that needs credentials must
  supply them, and those values sit outside the scanner's redaction
  guarantees. Never place a secret in a URL: embedded credentials are rejected,
  but a secret in a query string would not be.
- Wildcard allowlist entries such as `*.example.com` are permitted. Pin the
  allowlist to the provider's exact host when possible.

See the [security model](security/security-model.md#network-policy) and
[optional AI assistance](ai.md#network-policy-for-providers) for the
enforcement details.

## How to stay fully offline

Any one of these is sufficient:

```bash
# 1. Use deterministic, rule-based guidance (recommended for most users).
soroban-scan scan ./contracts --guidance

# 2. Keep assistance disabled in configuration.
[ai]
enabled = false

# 3. Build without the optional transport feature (the default).
cargo build --release -p soroban-scan-cli

# 4. Run in an environment with no outbound network access.
```

There is no automatic fallback from a disabled or failed provider to a network
request.

## What a repository-provided config can and cannot do

A `soroban-scan.toml` committed to a repository **can** set `[ai] enabled =
true` — the `[ai]` section is read from project config like any other setting.
It **cannot** cause a network request, because no provider is bundled in any
build.

It can still change the outcome of a `--guidance` run: with assistance enabled
and no bundled provider, the CLI fails closed with

```
error: assistance is unavailable: assistance provider 'openai' is not available in this build (see docs/ai.md)
```

and exits with status `2` after the deterministic report has been written. A
scan without `--guidance` never constructs an assistant and is unaffected.

## Consent checklist for operators

Before enabling a provider:

- confirm the `ai-http` feature and provider are intentional;
- pin the destination allowlist to the provider's exact host;
- verify the provider's retention and training terms;
- decide whether the `evidence` snippet may leave your environment;
- treat `.scanner-cache/ai` and any redirected stderr guidance as sensitive;
- confirm logs and crash reports do not capture payloads;
- document the opt-out path for developers who cannot use remote assistance.

If the scanner ever sends data outside this contract, treat it as a security
vulnerability and report it per [`SECURITY.md`](../SECURITY.md).
