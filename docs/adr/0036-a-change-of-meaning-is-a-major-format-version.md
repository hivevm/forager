# ADR-0036: A change of what a style document's field means is a major format version, and a condition is added only where an older reader fails closed

- **Status:** 🟢 accepted
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** `crates/style-ir` (the version rule of the style document and its schema check), `schema/`, and every change to a field of the style IR

## Context

[ADR-0020](0020-style-document-compatibility.md) sets the version rule by the shape of the
document: adding an optional field, an enum value, or a CQL2 function is a minor bump, any other
change of the shape a major one, and `crates/style-ir/tests/schema.rs` decides it by comparing
the derived schema with the checked-in file. The design revision of 2026-10-31 found two kinds of
change that rule does not see, both made in the work since the last revision while the format was
still unreleased and free to change:

- **A change of meaning without a change of shape.** `Style.line_offset` turned from "positive to
  the left" to "positive to the right" of a line; a bound label `size`, `priority`, `offset`, or
  `shadow_size` came to be rounded to an integer; what a `Bound::Binding` reads now depends on the
  property it binds. Each leaves the schema as it was, so after the first release a document
  written by one version would draw differently in the next, and the check would pass without a
  bump.
- **A new condition field.** `labels[].expression` was added as an optional field, a minor bump
  by the rule. A reader of the older minor drops it as unknown, with a warning, and draws every
  label the condition would have left out. [ADR-0022](0022-lenient-reader-path-to-error.md) and
  the convention that a reader skipping what it cannot read fails closed hold the opposite for a
  condition that does not read: it never matches. A dropped condition widens what a map draws.

Forces: G-4 asks that a newer minor be read with warnings; Q-4 asks that the meaning MapServer
gives a map survives; Q-9 and the fail-closed convention ask that what a lenient reader leaves out
never widens what a map draws or what an input may cost. The freeze of ADR-0020 applies from the
first release tag, which does not exist yet, so the rule can still be set before it binds.

Questions the maintainer settled on acceptance:

1. Is a change of meaning a major bump in every case, or only where a document written by the
   older version draws differently, so that a widened range of accepted values stays minor?
2. For a new condition field, which of these: (a) it is a major bump; (b) it is minor, but a
   condition is never a new top-level field and lives where an older reader already reads
   conditions, such as a CQL2 function inside an existing expression, which ADR-0020 already reads
   as never matching; (c) the document carries a list of fields an older reader must not drop,
   and dropping one makes the element it sits on never drawn.
3. Is the check for meaning a reviewed list, kept beside the schema, of the fields whose meaning
   moved since the last release, or only a line in the review skill's checklist?

The decision answers the first two: a change of meaning is a major version in every case, and a new
condition is minor only in the form of (b). The third is left to the check that enforces it.

## Decision

We will treat a change of what a field of the style document means, with or without a change of
its shape, as a major format version, and add a condition to the document only where a reader of
an older minor reads what it does not know as never matching.

**Out of scope:** the release process and its tags; how a migration step rewrites a field whose
meaning moved, which ADR-0020's chain of steps already decides; the meaning changes made before
the first release, which the freeze leaves free.

## Alternatives considered

- **Shape only, as ADR-0020 stands** — the check decides it alone, and a document of an older
  version silently draws differently, against Q-4; a dropped condition draws what it should not.
- **Every new optional field a major bump** — safe, and it gives up G-4's lenient reading of a
  newer minor for every addition, not only for those that widen a map.
- **A new field per meaning (`line_offset_right` beside `line_offset`)** — no bump, and the IR
  carries every meaning it ever had.

## Sources / Prior art

- [Semantic Versioning 2.0.0](https://semver.org/): a backwards-incompatible change of the public
  API, whatever its form, is a major version.
- JSON Schema and Protocol Buffers' guidance on unknown fields: an unknown field may be ignored only
  where ignoring it is safe; Protocol Buffers warns against reusing a field for a new meaning.
- The style document's own history between `4c42c42` and `6745da4` (the line offset's sign, the
  rounded label bindings, `labels[].expression`).

## Consequences

- Positive: a document of a released version keeps its meaning in every later minor; a newer
  minor's condition never widens an older reader's map.
- Negative / trade-offs: a change of meaning cannot be told from a shape and asks for a review
  step or a list kept by hand; a condition that could have been a small optional field may have
  to wait for a major or take the form of an expression.
- Follow-ups: how the review or a check names a change of meaning; the migration step of the
  first change of meaning after the freeze.

## Enforcement

**Not mechanically decidable:** a change of meaning leaves the schema as it was, so no comparison
of schemas sees it; the review of a change to `crates/style-ir/src/model.rs` asks whether a field's
meaning moved. The condition half is decided by a test of `style-ir` that reads a newer minor's
document with an unknown condition and expects the element it sits on never to match, citing this
ADR with `Verifies: ADR-0036`, once the form question 2 chooses exists.
