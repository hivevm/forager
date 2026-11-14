# Ticket: Read a label's font list with one reader

## What to build

A `FONT` list, `a,b` naming fallbacks and `ar:font` a script's prefix, is read twice, and the two
readers already disagree:

- the frontend's `check_fonts` (`crates/mapfile/src/translate/fonts.rs:77-81`) splits on every
  comma and warns "the font set has no font ``" of the empty entry after a trailing comma;
- `label::font::attempts` (`crates/label/src/font.rs:188-200`) takes no step for that entry.

The frontend holds the font set's names twice too, in `fonts.rs:36,55` (`first_written`) and in
`mod.rs:200` (`font_names`). The engine finds a font by a linear `eq_ignore_ascii_case`
(`crates/engine/src/fonts.rs:47-49`). `Map.fonts`' doc says no two names differ only in case, and
no reader of the style document checks it. ADR-0008 keeps `mapfile` from `label`, so the shared
reader lies lower, in `style-ir`. Read `check_single_font` at the pin first.

## Acceptance criteria

- [ ] Given `FONT "sans,"` and a font set that holds `sans`, when the mapfile loads, then no
  `font-unknown` is reported, by a test.
- [ ] Given the workspace, when the change lands, then one function splits a font list, and the
  frontend and `label` call it.
- [ ] Given a style document whose fonts hold two names that differ only in case, when it is read,
  then the reader decides it as the maintainer chose, by a test.

## Read first

- Touches: `crates/style-ir/src/model.rs`, `crates/mapfile/src/translate/fonts.rs`,
  `crates/mapfile/src/translate/mod.rs`, `crates/label/src/font.rs`, `crates/engine/src/fonts.rs`
- `docs/adr/0008-ports-and-adapters-crates.md`, `docs/adr/0031-label-glyphs.md`,
  `docs/adr/0036-c-ports-in-style-ir.md`

## Blocked by

- docs/plan/revision-2026-11-13/10-hash-table-reader-once.md

## Mode

with the human: the reader of the style document refusing or warning of a case-twin is a decision
on the format, which ADR-0006 keeps with the maintainer. The rest may run unattended once that is
decided.
