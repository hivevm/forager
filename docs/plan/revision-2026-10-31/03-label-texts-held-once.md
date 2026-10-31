# Ticket: Hold a label's text once per feature, not once per cached point

## What to build

A label's text is copied once per cached point on two paths of `crates/engine/src/labels.rs`.
Under `MINDISTANCE`, `cache` pushes a copy for every point it caches
(`texts.push(c_bytes(text).into())`); and with or without it, `finish` clones the text of every
cached point into the report `Rendered.labels` (`text: self.label_texts[…].to_string()`, since the
G-10 comparison). So one MultiPoint of 100,000 vertices with a text of 1 MiB holds about 100 GB,
from an input well under the data-file limit of Q-9; `MAX_LABELS` bounds the count, not the
bytes. The store of one text per definition and feature, which the cached points index, already
exists (`label_texts`, `Cached.text`): drop `texts`, let `same_text` compare through `Cached.text`,
and let the report share the text instead of cloning it per point. Add cases with and without
`MINDISTANCE` that read `Rendered.labels` to `crates/engine/tests/label_memory.rs`, whose texts
today are short. The convention that memory follows the input's size, in
[`docs/CONVENTIONS.md`](../../CONVENTIONS.md), is the measure.

## Acceptance criteria

- [ ] Given a MultiPoint of 100,000 vertices, a label of a 1 MiB text, and `MINDISTANCE 10`, when it
  is drawn under the counting allocator of `label_memory.rs`, then the peak allocation stays within
  a small multiple of the input's size.
- [ ] Given the same MultiPoint without `MINDISTANCE`, when it is drawn and `Rendered.labels` is
  read, then the peak allocation stays within a small multiple of the input's size.
- [ ] Given the cases of `crates/engine/tests/labels.rs` with `MINDISTANCE`, when they run, then
  they pass unchanged.

## Read first

- `cache` and `Members` in `crates/engine/src/labels.rs`; `crates/engine/tests/label_memory.rs`;
  [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md); the entry on readers of untrusted bytes
  in [`docs/CONVENTIONS.md`](../../CONVENTIONS.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: it is a resource limit of Q-9, which ADR-0007 keeps with the maintainer.
