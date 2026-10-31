# Ticket: Cite ADR-0036's condition half by the test that decides it

## What to build

[ADR-0036](../../adr/0036-a-change-of-meaning-is-a-major-format-version.md) was accepted on
2026-10-31 with a `**Not mechanically decidable:**` paragraph for its change-of-meaning half, and
says its condition half "is decided by a test of `style-ir` that reads a newer minor's document
with an unknown condition and expects the element it sits on never to match, citing this ADR with
`Verifies: ADR-0036`, once the form question 2 chooses exists". The maintainer chose form (b), a
condition in a CQL2 function inside an existing expression, and that form exists:
`a_newer_minors_unknown_operator_keeps_its_class_which_never_matches`
(`crates/style-ir/src/document.rs`) reads a document of minor 1.9 with an unknown operator and holds
its class to never match, citing ADR-0030 only. No test cites ADR-0036, and the traceability check
passes only on the paragraph's exemption. Cite it where the test decides all the half asks: a class,
a label, and a filter alike, as YAML and JSON.

## Acceptance criteria

- [ ] Given `scripts/check-traceability.sh`, when it runs, then a test with `Verifies: ADR-0036`
  holds a newer minor's unknown function in a class's expression, a label's, and a layer's filter
  to never match.
- [ ] Given that test, when the lenient reader is changed to drop the unknown expression, then it
  fails, shown once by hand.

## Read first

- ADR-0036, [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0030](../../adr/0030-cql2-expressions.md); `crates/style-ir/src/document.rs` and
  `lenient.rs`; the entry on what a `Verifies:` marker claims in `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the test and the traceability check decide its criteria; it adds a test and changes no
reader, so it touches neither the style document format nor its versioning, which
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer, and counts against its cap of
three; a wrong result shows as a mutation of the reader the test leaves green; stop: when a label or
a filter does not hold as a class does, the session records it in this file and hands off, since
that is a defect of the reader under ADR-0020.
