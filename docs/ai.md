# Optional AI Assistance

The scanner's deterministic engine is authoritative. Assistance is an optional,
clearly labeled layer that helps developers understand findings. It is **off by
default**, performs **no network access by default**, and can never change the
deterministic result.

## Guarantees

- The scanner is fully functional with assistance disabled.
- Assistance never creates, removes, suppresses, or overrides findings.
- Assistance never invents source locations or evidence.
- Assistance never claims certainty beyond the observed evidence.
- Assistance never accesses secrets, submits transactions, or modifies code.
- Assistance output is always labeled as assistance.

## Privacy and data flow

What can leave the machine, the exact data a provider receives, retention
caveats, and offline alternatives are documented in
[AI assistance privacy and data flow](ai-privacy.md). The short version: the
default build ships no provider and makes no network requests. A provider can
only reach finding context through the `Assistant` trait — never whole files,
environment variables, or credentials — and the exact fields are listed in
that document.

## Offline guidance (no model, no network)

`--guidance` prints deterministic, rule-based guidance derived solely from the
finding and its rule documentation:

```bash
soroban-scan scan ./contracts --guidance
```

Output starts with a summary and then one labeled explanation per finding:

```
GUIDANCE (offline, rule-based)

Scanned with 5 deterministic findings: critical 0, high 2, ...
```

This is not model-generated. It is a deterministic restatement of the rule's
description, evidence, and remediation, plus prioritization hints.

## Assistance cache

Repeated assistance requests for the same finding are cached by
**fingerprint + provider + model** (`crates/soroban-scan-core/src/ai/cache.rs`).
A cache hit avoids calling the provider again, which saves latency and (for a
model provider) tokens.

- Entries live under the scan cache directory (`.scanner-cache/ai`, excluded
  from discovery).
- The cache is content-addressed: keys are SHA-256 digests and filenames contain
  no evidence or source text.
- Only the rendered explanation is stored. Request payloads, environment
  variables, and whole source files are never cached. Note that the rendered
  explanation contains the finding's evidence snippet and file path, so the
  cache directory is as sensitive as the source it describes.
- Storage is bounded to `DEFAULT_MAX_ENTRIES` (256). When full, the
  least-recently-used entry is evicted deterministically (oldest access first,
  ties by key).
- Cache I/O is best-effort: a missing, corrupt, or read-only cache directory
  never breaks assistance or changes findings.


## Enabling a model-backed provider

Configuration has an `[ai]` section:

```toml
[ai]
enabled = false
# provider = "openai"
# model = "..."
temperature = 0.0
max_output_tokens = 1024
```

Model parameters have conservative defaults and hard bounds:

| Field | Default | Accepted range | Rationale |
| ----- | ------- | -------------- | --------- |
| `temperature` | `0.0` | `0.0`–`1.0` (finite) | Deterministic, reproducible assistance; high values increase nondeterminism. |
| `max_output_tokens` | `1024` | `1`–`8192` | Bounds latency and runaway provider cost. |

Values outside these ranges — including `nan` and infinities for
`temperature` — are rejected during configuration loading, before any scan or
provider call. The scan exits with a configuration error naming the field.

No model provider is bundled. If you set `enabled = true` without a bundled
provider — or name a provider that is not compiled in — no provider call is
made. With `--guidance` the CLI still completes the scan and falls back to the
offline, rule-based guidance, printing a warning:

```
warning: assistance is unavailable: assistance provider 'openai' is not available in this build; continuing with offline guidance
GUIDANCE (offline, rule-based)
```

This is deliberate: assistance is advisory, so an unavailable provider can never
change findings or the exit code. The output is clearly labeled as offline
guidance, and enabling assistance can never silently use unverified behavior.

## Request budget and backoff

Assistance is bounded so a scan with thousands of findings cannot trigger
thousands of provider requests:

```toml
[ai]
max_requests = 50   # hard per-scan budget; 0 disables calls
max_retries = 2     # bounded retries per request
backoff_ms = 250    # base exponential backoff
max_backoff_ms = 4000
failure_threshold = 3  # consecutive failures before assistance is disabled
```

`GuidanceSession` consumes one slot for the summary and one per finding. When
the budget is exhausted it stops calling the provider, prints an `AI-001`
diagnostic to stderr, and the scan completes normally (the deterministic report
and exit code are unchanged). Retries are capped by `max_retries` with
exponential backoff capped at `max_backoff_ms`; assistance never retries
indefinitely.

After `failure_threshold` consecutive provider failures the circuit breaker
opens for the remainder of the run: no further provider calls are made, an
`AI-002` diagnostic is printed, and the scan completes with the deterministic
result. A successful call resets the failure count.

## Implementing a provider

A provider implements the `Assistant` trait in
`crates/soroban-scan-core/src/ai/mod.rs`:

```rust
pub trait Assistant {
    fn name(&self) -> &str;
    fn explain_finding(&self, finding: &Finding, rule: Option<&RuleMetadata>) -> Explanation;
    fn summarize(&self, findings: &[Finding]) -> Explanation;
}
```

Requirements for any provider:

1. Set `ai_generated: true` and use [`AI_LABEL`](../crates/soroban-scan-core/src/ai/mod.rs)
   so output is clearly identified as model-generated.
2. Accept only existing findings and rule metadata as input. Never synthesize
   findings or locations.
3. Never include environment variables, secrets, or absolute user paths in
   output.
4. Obey the network policy below.

## Network policy for providers

The default build performs no network access. A provider that needs the network
must:

- Be explicitly opt-in and disabled by default.
- Enforce an allowlist of destinations.
- Enforce request timeouts and response-size limits.
- Validate URLs and reject unsafe redirects.
- Protect against SSRF (no access to internal endpoints or metadata services).
- Fail closed on any error.
- Never send source code or secrets off-device without explicit, informed
  user consent.

The non-default `ai-http` feature provides the audited transport in
`crates/soroban-scan-core/src/ai/http.rs`. It is absent from the default feature
set:

```toml
# crates/soroban-scan-core/Cargo.toml
[features]
default = []
ai-http = ["dep:ureq", "dep:url"]
```

A provider builds a [`NetworkPolicy`](../crates/soroban-scan-core/src/ai/http.rs)
with an explicit allowlist (bare host names with optional ports, for example
`api.example.com` or `*.example.com:443`) and hands it to `HttpClient`:

```rust
use soroban_scan_core::ai::http::{HttpClient, NetworkPolicy};
use std::time::Duration;

let policy = NetworkPolicy::new(["api.example.com"])?
    .with_limits(
        Duration::from_secs(5),   // connect
        Duration::from_secs(15),  // read headers/body
        1024 * 1024,              // hard response-size limit
        3,                        // redirect budget
    )?;
let client = HttpClient::new(policy);
let response = client.get("https://api.example.com/v1/respond")?;
let body: serde_json::Value = response.json()?;
```

The client enforces HTTPS, rejects embedded credentials, re-validates every
redirect target, resolves every destination and rejects loopback, private,
link-local, carrier-grade NAT, documentation, multicast, and metadata addresses,
and fails closed on malformed responses or non-2xx statuses. The allowlist
itself is validated when the policy is built: empty or wildcard-only entries,
private IPs, schemes, paths, and userinfo are rejected.

Provider output is passed through the deterministic redaction pass in
`ai::redact` before rendering. It removes common token formats, secret-like
assignments, PEM private keys, and absolute home directories; `Explanation`
applies the same pass again at render time as a final gate.

## What AI must never do

- Invent findings or source locations.
- Override or suppress deterministic findings.
- Claim certainty without evidence.
- Access secrets or execute actions.
- Modify code automatically.
