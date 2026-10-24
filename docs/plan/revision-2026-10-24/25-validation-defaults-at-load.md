# Ticket: Apply a `%name%`'s default from `VALIDATION` at load

## What to build

The specification's Non-Goals exclude "runtime substitution of `%var%`, beyond the default from
`VALIDATION` applied at load time": the default is in scope. No ticket carried it; #26 named the
gap in its doubts and asked where it belongs. Today a `%name%` in a `FILTER` or an `EXPRESSION`
is a warning and the condition keeps `false` (`docs/semantics/filter.md`), and `VALIDATION` is
read and dropped by the parser. msautotest files use both.

The frontend replaces each `%name%` for which a `VALIDATION` block of the layer, or of the map,
gives `default_name`, as MapServer applies its default substitutions when it loads a map, in the
parameters MapServer substitutes them in; a `%name%` without a default stays as today. The
session reads MapServer's default substitution at the pinned release first, confirms the
function and where it applies, and cites it on a semantics page.

## Acceptance criteria

- [ ] Given a layer with `VALIDATION "default_v" "3" END` and `FILTER ([v] = %v%)`, when it is
  loaded, then the filter is the tree of `([v] = 3)` and no warning names `%v%`.
- [ ] Given a `%w%` without a default, when it is loaded, then it is the warning it is today and
  the condition keeps `false`.
- [ ] Given a default that would make an expression MapServer cannot parse, when it is loaded,
  then it is that expression's warning, as for one written so.

## Read first

- `docs/SPECIFICATION.md` (Non-Goals), [ADR-0019](../../adr/0019-cql2-expressions.md); the
  convention on behaviour ported from MapServer or PROJ in `docs/CONVENTIONS.md`.
- `docs/semantics/filter.md`, `docs/semantics/expression.md`, `crates/mapfile/src/expression.rs`,
  `crates/mapfile/src/syntax.rs` (`VALIDATION`).

## Blocked by

- docs/plan/revision-2026-10-24/17-translation-split-by-block.md

## Mode

with the human: which parameters take the default, and whether `DATA` and `CONNECTION` do, is a
scope question for the maintainer, and the result changes what the style document holds.
