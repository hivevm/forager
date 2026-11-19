# Ticket: The comments that numbering left short are wrapped again

## What to build

`scripts/number-adrs.sh` replaces each draft identifier with its number by `sed`. The draft
identifier of ADR-0050, `ADR-draft-` and a slug of six words, was some 40 characters longer than
its number. A paragraph wrapped around a draft identifier is left with a short line where it
stood. Commit 458ac7f9 left such lines, for example:

- `crates/symbolize/Cargo.toml`: "(ADR-0050); its default features are off, so it brings no" /
  "threads and no triangulation.";
- `crates/symbolize/src/geometry.rs` and `crates/symbolize/src/layer_transform.rs`: the module
  docs break after "(ADR-0050), and" and "(ADR-0050),";
- `scripts/check-notice.sh`: "follows JTS (ADR-0050)" on a line of its own;
- `scripts/lib/notice.sh`: "(ADR-0050): the copyright line of those sources, and the".

It touched 16 files: code, tests, and `fuzz/fuzz_targets/render.rs` among them, and Markdown
under `docs/` (`ARCHITECTURE.md`, three backlog entries, `semantics/style-symbol.md`). It touched
no ADR. The spec of backlog-deviations is deleted since.

The comment width check catches a line that is too long, not one that is too short. Wrap each
paragraph that 458ac7f9 shortened at 100 characters again, in every file of it that still
exists. Earlier numberings, such as f502af2b, are out of scope; a short line of theirs that the
session meets is named in the hand-off. This is a change of comments and prose only. It touches
no ADR and changes no word.

## Acceptance criteria

- [ ] Given each file of `git show --stat 458ac7f9` that still exists, when the paragraphs around
  the identifiers it replaced are read, then no line but a paragraph's last ends more than 20
  characters short of 100, where the next word would have fit.
- [ ] Given the diff of the change, when `git diff --word-diff` reads it, then it changes no word,
  only line breaks.
- [ ] Given the tree after the change, when `scripts/check-all.sh` runs, then it passes, and the
  notice checks still read every header they parse.

## Read first

- Touches: the files of `git show --stat 458ac7f9` that still exist
- `scripts/check-comment-width.sh`, and check 15 of `scripts/check-docs.sh` (the width of
  Markdown prose)
- `scripts/check-notice.sh`: it parses the provenance lines of a notice header, so a header is
  wrapped only where the check still reads it

## Blocked by

- None (can start immediately)

## Mode

unattended. The word diff and the checks decide it, and a wrong result costs one revert. Stop
at a paragraph whose wrapping a check refuses. Leave it as it stands, and name it in the
hand-off.
