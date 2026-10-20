# Diagnostics

Diagnostics are **not** security findings. They report something the scanner
could not do — a file it skipped, a manifest it could not read, a rule whose
output was rejected — so a scan can keep going and still tell you what was
missed. Findings are what fail the gate; diagnostics never do.

Every diagnostic carries a **stable code** and an explicit **level**, so tooling
can classify or suppress them without parsing the human-readable message.

## Fields

| Field | Description |
| ----- | ----------- |
| `code` | Stable identifier, for example `PARSE-001`. Never reused for a different meaning. |
| `level` | One of `info`, `warning`, `error`. |
| `message` | Human-readable description. May change between releases. |
| `path` | Related file, when there is one. |
| `line` | Related line, when there is one. |

## Codes

| Code | Level | Meaning |
| ---- | ----- | ------- |
| `PARSE-001` | warning | A source file could not be parsed; it was analyzed only as raw text. |
| `MANIFEST-001` | warning | A `Cargo.toml` could not be read or parsed; it was skipped. |
| `SOURCE-001` | warning | A source file exceeded the size limit and was skipped. |
| `SOURCE-002` | warning | A source file could not be read and was skipped. |
| `RULE-001` | error | A rule produced a location that is not a safe relative path; the finding was rejected. |
| `RULE-002` | error | A rule reported a file outside the analyzed set; the finding was rejected. |
| `RULE-003` | error | A rule produced a finding without evidence; the finding was rejected. |
| `CONFIG-001` | warning | A severity or confidence override has no effect because its rule is unknown or disabled. |
| `BASELINE-001` | warning | A baseline references a rule id that is no longer registered; the entry remains forward compatible. |

Codes are grouped by area: `PARSE-*`, `MANIFEST-*`, `SOURCE-*`, `RULE-*`,
`CONFIG-*`, and `BASELINE-*`. `config validate` prints `CONFIG-*` warnings to
stderr while still exiting successfully, because they do not prevent a scan from
running.

## JSON output

Diagnostics appear under `diagnostics` in `--format json`:

```json
{
  "code": "PARSE-001",
  "level": "warning",
  "message": "failed to parse source: expected an expression, found `{`",
  "path": "src/lib.rs"
}
```

The terminal report prints each diagnostic with its code (`- [PARSE-001] ...`),
and `--verbose` echoes them to stderr with the same prefix.

## Stability

Once a code is released it keeps its meaning. New codes may be added; existing
codes are not renamed or repurposed. Treat the code and level as the stable
interface and the message as informational.
