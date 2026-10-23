# Import Resolution

`ParsedSource::resolve_path` canonicalizes a rendered path string against the
`use` items in the **same file**. It lets detectors reason about the type behind
a local name instead of matching the literal receiver text.

The resolver is deliberately small and syntactic. It never loads another file,
never expands a macro, and never executes or compiles the scanned code.

## What is resolved

| Form | Example | Canonical result |
| --- | --- | --- |
| Plain import | `use soroban_sdk::Env;` → `Env` | `soroban_sdk::Env` |
| Renamed import | `use soroban_sdk::token::Client as Token;` → `Token` | `soroban_sdk::token::Client` |
| Grouped import | `use soroban_sdk::{storage::Instance as Store, Env};` | `soroban_sdk::storage::Instance`, `soroban_sdk::Env` |
| `self` import | `use soroban_sdk::storage::{self, Instance};` | `soroban_sdk::storage` |
| Suffix | `Token::new` with the alias above | `soroban_sdk::token::Client::new` |
| Glob | `use soroban_sdk::storage::*;` → `Instance` | ambiguous: `soroban_sdk::storage::Instance` |

Imports inside inline `mod` blocks are collected as well. The map is per file,
so imports from a parent module or another file are not visible.

## What is not resolved

- **Cross-file and cross-crate re-exports.** `pub use` chains are not followed.
- **Type aliases.** `type Store = soroban_sdk::storage::Instance;` is not an
  import and is not tracked.
- **Local variable types.** `let store: Store = env.storage().instance();` does
  not make `store` resolve to `Store`.
- **Method receivers that are expressions.** `env.storage().instance()` is not a
  path, so it is returned unchanged.
- **Macro expansion, generics, and traits.** Only plain identifier segments are
  parsed; anything containing `<`, `>`, or other syntax is left untouched.
- **Conditional compilation.** `#[cfg(...)]` is not evaluated; every `use` item
  is collected regardless of the active configuration.

## Ambiguity

The resolver returns one of three outcomes:

- `Resolved(path)` — exactly one canonical path matched.
- `Ambiguous(candidates)` — a glob matches, or the same local name is imported
  more than once. Candidates are sorted and deduplicated.
- `Unresolved` — no import owns the first segment; the path was already
  qualified or is a local name.

Callers must not guess. `ImportMap::canonicalize` returns the original text for
ambiguous and unresolved paths. A detector that needs to classify an ambiguous
name can inspect every candidate with `Resolution::any_candidate`. Detectors
should fail toward reporting: if any candidate looks like the sensitive type
(for example instance storage), the call is still treated as that type.

## Detector usage

SS-016 (unbounded instance storage) canonicalizes call contexts through the
import map before deciding whether a call targets instance storage. This catches
both of these forms:

```rust
use soroban_sdk::storage::Instance as Store;
Store::set(&store, &DataKey::A, &a);
```

```rust
use soroban_sdk::storage::*;
Instance::set(&store, &DataKey::A, &a);
```

The corpus fixtures `fixtures/corpus/SS-016/positive_alias.rs` and
`positive_glob.rs` pin both forms.

## Security notes

Resolution is used only to widen evidence, never to suppress a finding. A wrong
or missing import therefore cannot hide a storage call from SS-016: ambiguous
paths are checked against every candidate and unresolved paths fall back to the
literal text.
