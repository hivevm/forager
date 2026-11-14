# Ticket: Read directive keywords, CONNECTIONTYPE, and a VALIDATION pair by token value

## What to build

From `docs/backlog/keywords-matched-by-token-value.md`. MapServer's loaders switch on the lexer's
token value, where the frontend still matches by the word in three places:

- **A directive or block keyword.** Two keywords of one value are one statement: `SIMPLE`,
  `MS_SYMBOL_SIMPLE` (1000), inside a `STYLE` is `ANGLE` (1000), and `POSTLABELCACHE` inside a
  `LABEL` is `FORCE` (both 1098). The parser (`crates/mapfile/src/syntax.rs`) matches statements by
  name.
- **`CONNECTIONTYPE`.** `loadLayer` reads it with `getSymbol(14, MS_OGR, …)`, so `small` (7) is
  `MS_WMS`; the frontend keeps the word (`translate/mod.rs`, the layer's source and the graticule
  check). At the pin `graticule` lexes as `GRATICULE` (1140), not `MS_GRATICULE`, so MapServer
  refuses `CONNECTIONTYPE graticule` itself.
- **A `VALIDATION` pair of a bare keyword.** `loadHashTable` reads both with `getString`, failing
  the load; the frontend drops the pair without a diagnostic (`translate/defaults.rs`).

Read each loader at the pin first, then match by the token value of `keywords.rs`
(`docs/semantics/tokens.md`), as a value's keyword choice already does.

## Acceptance criteria

- [ ] Given `SIMPLE 30` inside a `STYLE`, when the mapfile loads, then the style's angle is 30, as
  MapServer reads it, by a test that cites the loader's lines.
- [ ] Given `POSTLABELCACHE TRUE` inside a `LABEL`, when it loads, then the label is forced.
- [ ] Given `CONNECTIONTYPE small`, when it loads, then the layer reads as MapServer's `MS_WMS`
  reads, and given `CONNECTIONTYPE graticule`, then the load reports what MapServer refuses.
- [ ] Given a `VALIDATION` pair of a bare keyword, when it loads, then it is reported, by a test
  that asserts the code.

## Read first

- Touches: `crates/mapfile/src/syntax.rs`, `crates/mapfile/src/keywords.rs`,
  `crates/mapfile/src/translate/mod.rs`, `crates/mapfile/src/translate/defaults.rs`,
  `docs/semantics/tokens.md`
- `docs/CONVENTIONS.md`, *Behaviour ported from MapServer or PROJ is read in their source*.

## Blocked by

- docs/plan/revision-2026-11-13/13-budget-narrowed.md
- docs/plan/revision-2026-11-13/12-style-transform-warned-once.md

## Mode

unattended: each criterion is a test of the loader's behaviour; one revert. Stop when the C at the
pin reads otherwise than this ticket says, and hand off with what it reads.
