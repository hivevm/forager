# Ticket: Review the MapServer ports that merged without a fresh-context review

## What to build

Of the pull requests of the spec `ir-completeness`, only #37 and #52 ran the fresh-context review
of [`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done); #24 to #33 record
"No fresh-context review ran: subagents are rate-limited". Both reviews that ran found the port
deviating silently from its source on cases the msautotest corpus does not exercise: `AUTO`
misdescribed and matched on the first word (#37); the `init=` code split at the first `:`, the
last duplicate definition kept, a definition cut at the line end (#52). #31 records one more,
caught by its author: the sign of a negative `GAP`. The ports of the unreviewed pull requests are
as likely to hold such cases.

A session in a fresh context reviews, area by area, each rule the frontend ports against the
MapServer source at the pinned release, by the function its doc comment or semantics page cites,
and writes a test for each branch of that function the corpus does not take:

1. expressions, `CLASSITEM`, `FILTER`, `FILTERITEM` (#26): `crates/mapfile/src/expression.rs`
   against `src/maplexer.l`, `src/mapparser.y`, `msEvalExpression`, `msEvalTextExpression`;
2. bindings and colours (#28): `loadColor`, `bindStyle`;
3. styles (#31): `loadStyle`, `msUpdateClassScaleFactor`, `computeSymbolStyle`;
4. symbols and symbol sets (#32): `loadSymbol`, `loadSymbolSet`, `msGetSymbolIndex`;
5. labels and the font set (#33): `loadLabel`, `msShapeGetLabelAnnotation`, `msLoadFontSet`;
6. the base context of `--base` and its file loader (#25): `crates/cli/src/files.rs` against Q-8.

A deviation the session confirms gets a regression test and its fix in the same change when it
is small, and a ticket of its own otherwise; a page under `docs/semantics/` that misstates the
source is corrected with it.

## Acceptance criteria

- [ ] Given each area above, when the session closes it, then its pull request lists each
  function read, with the release, and each branch tested or the reason it is not.
- [ ] Given a deviation found, when its fix lands, then a test that failed before it passes.
- [ ] Given the areas left, when the session stops, then this ticket names only those.

## Read first

- The conventions on behaviour ported from MapServer or PROJ and on MapServer's notice in
  `docs/CONVENTIONS.md`; the skill `review`.
- `docs/semantics/`, `scripts/fetch-msautotest.sh` (the pinned release),
  [ADR-0019](../../adr/0019-cql2-expressions.md).
- The pull requests named above (`gh pr view <n>`), for their doubts.

## Blocked by

- None (can start immediately)

## Mode

with the human: a confirmed deviation changes what the style document holds, whose format is
kept with the maintainer, and area 6 is the resolution of resource paths (Q-8)
([ADR-0007](../../adr/0007-harness.md)). Stop: one area per session; the ticket stays with the
areas left.
