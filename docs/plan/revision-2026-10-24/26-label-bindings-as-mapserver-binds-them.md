# Ticket: Bind a label's properties as MapServer binds them

## What to build

Found while revision ticket 22 read `docs/semantics/binding.md` against MapServer `rel-8-6-6`.
`bindLabel` in `src/maputil.c` binds a label's `SIZE`, `PRIORITY`, `SHADOWSIZE`, and `OFFSET` with
`bindIntegerAttribute`, `MS_NINT(atof(value))`, and reads `ALIGN` and `POSITION` as an integer
or, where that is 0, as a keyword compared without case. MapServer binds no `SHADOWCOLOR`:
`loadLabel` (`src/mapfile.c`, `case (SHADOWCOLOR)`) reads it with `loadColor(…, NULL)`, which
takes no binding. The frontend (`crates/mapfile/src/translate.rs`, `label`) carries
`SHADOWCOLOR [attr]` as a binding, where MapServer refuses the mapfile, and the IR's `Bound`
says a bound number is read with `atof`, without the rounding a label's integer properties get.

The frontend refuses a `SHADOWCOLOR` binding as MapServer does, and the IR states, for the
label properties MapServer binds as integers, that a bound value is rounded as `MS_NINT` rounds
it, and how `ALIGN` and `POSITION` read a bound value; `docs/semantics/binding.md` then drops its
note on the difference.

## Acceptance criteria

- [ ] Given `LABEL SHADOWCOLOR [c] END`, when it loads, then it is the error MapServer's loader
  gives for a colour that takes no binding, at its span.
- [ ] Given a label with `SIZE [s]`, `PRIORITY [p]`, `OFFSET [x] [y]`, `SHADOWSIZE [a] [b]`, when
  the IR is read, then the documentation of each says the bound value is rounded to an integer.
- [ ] Given `ALIGN [a]` and `POSITION [p]`, when the IR is read, then it states the integer and
  keyword reading of `bindLabel`, with its source.

## Read first

- `docs/semantics/binding.md`, `docs/semantics/label.md`; `crates/style-ir/src/model.rs`
  (`Bound`, `Label`); `crates/mapfile/src/translate.rs` (`label`); the convention on behaviour
  ported from MapServer or PROJ in `docs/CONVENTIONS.md`.
- MapServer `rel-8-6-6`: `bindLabel` and `bindIntegerAttribute` in `src/maputil.c`, `loadLabel`
  in `src/mapfile.c`.

## Blocked by

- None (can start immediately)

## Mode

with the human: what a label binding means is part of the style IR's format
([ADR-0007](../../adr/0007-harness.md)).
