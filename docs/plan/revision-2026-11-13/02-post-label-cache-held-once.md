# Ticket: Hold "a POSTLABELCACHE layer's labels are never cached" in one place

## What to build

The rule is applied twice. The frontend clears the flag at
`crates/mapfile/src/translate/mod.rs:516` (`layer.label_cache &= !layer.post_label_cache`), and
the engine decides it again at `crates/engine/src/lib.rs:194`
(`!layer.label_cache || layer.post_label_cache`). A style document written by hand may say
`post_label_cache: true` with `label_cache: true`, and only the engine's rule makes it right. Keep
the engine's rule, which holds for every document, and drop the frontend's line. The model's doc
comment of `label_cache` then says that `post_label_cache` overrides it. A single enum in place of
the two flags is the other shape; it changes the style document's shape, so it is the
maintainer's to choose.

## Acceptance criteria

- [ ] Given a style document with `post_label_cache: true` and `label_cache: true`, when its point
  layer renders, then its labels are drawn at once after the label cache, as for a mapfile's.
- [ ] Given the frontend, when it translates `POSTLABELCACHE TRUE`, then the rule is written once
  in the workspace (`rg 'post_label_cache' crates/mapfile/src` finds no second decision).

## Read first

- Touches: `crates/mapfile/src/translate/mod.rs`, `crates/engine/src/lib.rs`,
  `crates/style-ir/src/model.rs`, `schema/style-document-1.0.json`
- `docs/semantics/` page of `POSTLABELCACHE`, if one exists.

## Blocked by

- None (can start immediately)

## Mode

with the human: the doc comment of a field of the style document changes its schema's description,
and the format is kept with the maintainer by ADR-0006; the enum is a format decision.
