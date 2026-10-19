# Rule Cookbook

This cookbook works through three real detectors from evidence to judgment. Each
example uses the same fixtures the test suite uses, so you can run the scanner
and compare the output with the explanation.

The three examples cover the main detector styles:

| Style | Rule | What the detector observes |
| ----- | ---- | -------------------------- |
| Heuristic | [SS-006](#heuristic-ss-006) | A pattern that is suspicious but may be intentional |
| Structural | [SS-001](#structural-ss-001) | The shape of a contract entry point and its helpers |
| Metadata-based | [SS-007](#metadata-based-ss-007) | The value of a string literal |

Read [the rule system](README.md) first for severity and confidence semantics,
and [Writing a Security Rule](../contributors/writing-rules.md) for the full
contribution process.

## Heuristic: SS-006

**Question.** Does an entry point write to persistent storage without extending
the entry's TTL in the same function or in an intra-file helper?

**Evidence.** Open
[`fixtures/corpus/SS-006/positive.rs`](../../fixtures/corpus/SS-006/positive.rs):

```rust
#[contractimpl]
impl Vault {
    pub fn set_admin(env: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&symbol_short!("ADMIN"), &new_admin);
    }
}
```

The scanner records two facts for `set_admin`:

1. `persistent().set(...)` is a persistent-storage mutation.
2. No `extend_ttl`, `set_ttl`, or `bump` call appears in the function or in a
   same-file helper it calls.

Both facts together trigger SS-006 at the line of the `set` call.

**Why the confidence is low.** The storage write is certain; what it means is
not. TTL may be extended by a separate maintenance entry point, a helper in
another file, or an operational process. The detector only sees the analyzed
file, so it reports `confidence: low` and asks a human to confirm the TTL
policy.

**Where the line is.** The negative fixture,
[`fixtures/corpus/SS-006/negative.rs`](../../fixtures/corpus/SS-006/negative.rs),
performs the same write and then calls `extend_ttl`:

```rust
let key = symbol_short!("ADMIN");
env.storage().persistent().set(&key, &new_admin);
env.storage().persistent().extend_ttl(&key, 100, 1000);
```

The detector sees the TTL-management call and stays silent. A write to
`instance()` storage is also ignored because instance entries have different
TTL semantics and are outside the scope of this rule.

**Judgment call.** Do not raise the confidence just because the finding is
high-impact. A heuristic with real false positives should stay `low` (or
`medium` when the evidence is stronger) even when the potential impact is high.
Suppress reviewed cases with an inline directive and a reason instead of
weakening the detector for everyone.

## Structural: SS-001

**Question.** Does a public contract entry point mutate storage without any
`require_auth` or `require_auth_for_args` in its body or in the intra-file
helpers it calls?

**Evidence.** Open
[`fixtures/corpus/SS-001/positive.rs`](../../fixtures/corpus/SS-001/positive.rs).
The `set_admin` and `withdraw` functions are public, write storage, and never
authorize a caller:

```rust
pub fn withdraw(env: Env, to: Address, amount: i128) {
    let balance: i128 = env
        .storage()
        .persistent()
        .get(&symbol_short!("BAL"))
        .unwrap_or(0);
    let new_balance = balance - amount;
    env.storage()
        .persistent()
        .set(&symbol_short!("BAL"), &new_balance);
    let _ = to;
}
```

The detector does not look for a particular spelling of the function name. It
builds a small summary of each function (storage mutations, calls, and
authorization checks), follows same-file helper calls up to a bounded depth, and
then asks whether the merged facts contain a storage mutation and no
authorization check.

**Why the confidence is medium.** The structural evidence is strong: a public
entry point that changes state without an authorization call is a real risk.
It is not certain, because authorization may be enforced by a callee in another
file or contract, or the operation may be intentionally permissionless.

**Where the line is.** The negative fixture,
[`fixtures/corpus/SS-001/negative.rs`](../../fixtures/corpus/SS-001/negative.rs),
shows two ways to satisfy the check:

```rust
pub fn set_admin(env: Env, admin: Address, new_admin: Address) {
    admin.require_auth();
    // ...
}
```

and the helper form:

```rust
pub fn release(env: Env, admin: Address, to: Address, amount: i128) {
    Self::authorize(&admin);
    // ...
}

fn authorize(admin: &Address) {
    admin.require_auth();
}
```

The helper summary is what connects `release` to `require_auth`. Without that
step the detector would produce a false positive for every contract that
centralizes authorization. Read-only entry points such as `balance` are also
ignored because there is no mutation to protect.

**Judgment call.** Prefer structural facts over name matching. A detector that
looks for `require_auth` in the entry point itself is easy to write and easy to
fool; a helper-aware summary is harder and much more useful. Add a negative
fixture for every legitimate pattern you discover during review.

## Metadata-based: SS-007

**Question.** Does a string literal have the shape of a Stellar account (`G...`)
or contract (`C...`) address?

**Evidence.** Open
[`fixtures/corpus/SS-007/positive.rs`](../../fixtures/corpus/SS-007/positive.rs):

```rust
#[contractimpl]
impl Config {
    pub fn admin_address(env: Env) -> &'static str {
        let _ = env;
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    }
}
```

The detector reads string literals from the parsed file and checks the strkey
shape: the leading `G` or `C` and the expected length. It does not validate the
checksum, resolve the address, or query the network.

**Why the confidence is low.** The literal is definitely an address-shaped
string. Whether it is a security problem is a design question: a compile-time
constant may be deliberate, and the address is public rather than a secret.
Hardcoding prevents rotation and can leave a contract pointing at a compromised
or retired counterparty, so the finding is worth review, but it is not proof of
a vulnerability.

**Where the line is.** The negative fixture,
[`fixtures/corpus/SS-007/negative.rs`](../../fixtures/corpus/SS-007/negative.rs),
uses an ordinary string:

```rust
"hello from soroban"
```

The unit tests add a near-miss string starting with `X`; shape checking keeps
that out. Comments and identifiers are not string literals, so they do not
trigger the rule.

**Judgment call.** State exactly what the metadata check does and does not do.
Do not claim that an address is valid, malicious, or reachable. When a literal
is intentional, document the rotation plan and suppress the finding with a
reason rather than deleting the check.

## Applying the pattern to a new rule

Before writing code, write down:

1. **Problem.** What can go wrong, and for whom?
2. **Evidence.** Which AST facts prove the pattern is present?
3. **Limitations.** What can the detector not see (other files, runtime values,
   macros, external calls)?
4. **False positives.** Which legitimate patterns must stay silent? Add each one
   as a negative fixture.
5. **Confidence.** Given the evidence and the false-positive rate, what is the
   honest confidence?

Then implement the detector, add a positive and at least one negative fixture,
and document the rule. The corpus and quality tests will exercise the rule
automatically. See [the testing guide](../development/testing.md) for the
fixture layout and [the rule template](TEMPLATE.md) for the document structure.
