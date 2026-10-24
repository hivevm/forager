# Ticket: Bring two semantics pages back to what the frontend carries

## What to build

Two pages under `docs/semantics/` describe an earlier state of the frontend:

- `style-symbol.md` says `labelpoint` and `labelpoly` are among the transforms "not yet" carried
  over. `Translator::geom_transform` (`crates/mapfile/src/translate.rs:797-832`) carries both,
  and `label.md` describes them.
- `binding.md` lists `OFFSET`, `OPACITY`, `OUTLINEWIDTH`, `SYMBOL`, and `POLAROFFSET` as bindable
  "with later parameters"; the styles ticket made them bindings, and the labels ticket added
  bindings of a label's `FONT`, `SIZE`, `COLOR`, `OUTLINECOLOR`, `SHADOWCOLOR`, `SHADOWSIZE`,
  `OFFSET`, `POSITION`, `ALIGN`, and `PRIORITY`, which the page does not name.

The two pages say what the code does; the rest of the pages were read against the code in the
design revision of 2026-10-24 and hold.

## Acceptance criteria

- [ ] Given `style-symbol.md`, when it is read against `geom_transform`, then every transform the
  function carries is named as carried, and only the others as not yet.
- [ ] Given `binding.md`, when it is read against the `bound_*` calls of `translate.rs`, then
  every property that takes a binding is named, by the block it stands in.

## Read first

- `docs/semantics/README.md`, `docs/semantics/style-symbol.md`, `docs/semantics/binding.md`,
  `docs/semantics/label.md`.
- `crates/mapfile/src/translate.rs` (`style`, `label`, `geom_transform`).

## Blocked by

- None (can start immediately)

## Mode

with the human: no test decides what a semantics page says, so the maintainer reads it.
