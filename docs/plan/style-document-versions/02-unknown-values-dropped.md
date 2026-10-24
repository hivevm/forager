# Ticket: Drop a newer minor's unknown enum values and expression operators with a warning

## What to build

A document of a newer minor that holds an enum value the reader does not know — a render mode, a
layer status, a symbol type — loads with that field unset and a warning naming the value and its
path. An expression with an unknown operator or function loads; its class or filter is kept and
never matches, with the same warning. In a document of the current or an older minor, both are
load errors.

## Acceptance criteria

- [ ] Given a `1.9` document with `render_mode: hologram`, when it is read, then the layer's
  render mode is unset and a warning names `hologram` at its path.
- [ ] Given a `1.9` document whose class expression uses an operator `s_buffer`, when it is read,
  then the class is kept, never matches, and a warning names the operator and its path.
- [ ] Given either in a `1.0` document, when it is read, then it is a load error naming the path.
- [ ] Given each, when the document is written again, then the dropped field does not reappear.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0019](../../adr/0019-cql2-expressions.md); ticket 01's reader.

## Blocked by

- docs/plan/style-document-versions/01-unknown-fields-named-by-path.md
- docs/plan/ir-completeness/01-class-expressions-and-layer-filters.md

## Mode

with the human: the style document's versioning is kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
