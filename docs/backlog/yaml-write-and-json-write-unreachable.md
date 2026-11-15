# The codes `yaml-write` and `json-write` cannot be reached

**Observed:** fidelity ticket 03 (2026-11-15) looked for a style document that
`StyleDocument::to_yaml` or `to_json` in `crates/style-ir/src/document.rs` cannot write. None
exists, so no test names either code. `scripts/sensor-diagnostics-untested.sh` keeps listing both.

**Why neither call can fail:**

- `serde_json::to_string_pretty` fails only in three ways:
  - a map key that is not a string;
  - a `Serialize` that reports an error;
  - an I/O error.

  The style IR has none of them. Every map is a `BTreeMap<String, _>`. The impls of its own types
  (`Rgba`, `Span`, `Fonts`, the transform trees) only forward to strings, maps, and sequences.
  The writer is in memory. A number that is not finite is written as `null`, not refused.
- `serde_saphyr::to_string` 2.0.0 errs in these places:
  - on tags, anchors, and its own wrapper types such as `SingleQuoted`, which the IR does not use;
  - on an indentation indicator outside 1–9, which it handles itself by quoting;
  - on an indentation past `usize`;
  - on a `Serialize` that reports an error, and on a write error. A map key that is not a scalar
    it writes as an explicit `? key`, rather than refusing it.

  None of them applies to a style document.

**Is the code dead?** For the types of today, yes. The `Result` and its code guard a future type
whose `Serialize` can fail, or a change of the serializer. A test could reach them only through a
hook in production code, which the ticket rules out.

**What a decision weighs:**

- Option 1: keep the codes as they are. The sensor then lists them for good, and each review
  reads this entry.
- Option 2: have `to_yaml` and `to_json` return `String` with an `expect` that names this
  reasoning. This changes the API of `forager-style-ir` and its callers in the CLI.
