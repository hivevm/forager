# Ticket: Read MapServer's key-value blocks with one reader

## What to build

Two readers walk a block of string pairs, keys matched without case and the later pair winning, as
MapServer's `loadHashTable` reads them:

- `Defaults::of` (`crates/mapfile/src/translate/defaults.rs:38-63`) for `VALIDATION`, which warns
  of a pair it cannot read;
- `metadata` (`crates/mapfile/src/translate/mod.rs:85-98`) for `METADATA`, which drops the same
  pair silently.

`Translator::map` also filters the `WEB` blocks twice (`mod.rs:249` and `mod.rs:307`), and the
layers' fallbacks of `MAXFEATURES` travel as a positional tuple (`max_features_fallback:
(Option<i32>, i32)`, `mod.rs:239,254,541`). Read both kinds of block with one reader that reports
a pair it cannot read the same way. Bind the `WEB` blocks once, and name the fallback's parts.
Read `loadHashTable` at the pin for what MapServer does with a malformed pair.

## Acceptance criteria

- [ ] Given a `METADATA` block with a key and no value, when the mapfile loads, then it reports the
  pair as a `VALIDATION` block's is reported, by a test that asserts the code.
- [ ] Given the suite at the base otherwise, when the reader is one, then it passes unchanged.

## Read first

- Touches: `crates/mapfile/src/translate/defaults.rs`, `crates/mapfile/src/translate/mod.rs`
- `docs/CONVENTIONS.md`, *Behaviour ported from MapServer or PROJ is read in their source*, and
  *Diagnostic codes are kebab-case and stable*.

## Blocked by

- None (can start immediately)

## Mode

with the human: `Defaults::of` holds the 16 MiB bound of the defaults (Q-9), and the resource
limits are kept with the maintainer by ADR-0006.
