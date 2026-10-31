# Ticket: Hold a style document's `resolution` and `def_resolution` to the range the frontend reads

## What to build

Since #136 the engine scales every style and label by `resolution` over `def_resolution`
(`scale_factor` in `crates/engine/src/classes.rs`). The mapfile frontend reads both within 10 to
1000 (`crates/mapfile/src/translate/mod.rs`), but the style document's readers take any number
(`crates/style-ir/src/model.rs`): `def_resolution: 0` gives an infinite factor, a line's width then
comes out NaN through `ms_max(inf, 0 * inf)` and the stroke is dropped, hatches are refused, and the
labels' casts saturate; a negative one mirrors every size. No host renders a style document today,
so only the library reaches it, but the format is the browser's and a host's next input. #136 named
this as unverified; the survey of this revision read it in the code. Questions for the maintainer:
is the range part of the format (a document outside it is refused, or read leniently with the field
dropped and a warning, under ADR-0020's reader), or does the engine bound the factor where it
computes it, as MapServer's loader bounds `RESOLUTION` and `DEFRESOLUTION` at load; and is a
range that narrows what a document of today may say a change of meaning under ADR-0036.

## Acceptance criteria

- [ ] Given a style document with `def_resolution: 0`, `-72`, or `1e9`, when it is read and
  rendered by the chosen path, then it is refused or drawn at a factor within the frontend's range,
  with a diagnostic, a test per value.
- [ ] Given every document of `crates/style-ir`'s tests and the fixtures under `schema/`, when they
  are read, then they read as today.

## Read first

- [ADR-0014](../../adr/0014-style-document-serde.md),
  [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0036](../../adr/0036-a-change-of-meaning-is-a-major-format-version.md);
  `crates/style-ir/src/model.rs`, `lenient.rs`; `scale_factor` in `crates/engine/src/classes.rs`;
  `msLoadMap`'s reading of `RESOLUTION` in MapServer's `src/mapfile.c` at the pin; #136's doubts.

## Blocked by

- None (can start immediately)

## Mode

with the human: the style document format and its versioning stay with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
