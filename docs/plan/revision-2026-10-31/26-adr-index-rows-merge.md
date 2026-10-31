# Ticket: Let ADR branches cut in parallel add their index rows without a conflict

## What to build

Branches that propose ADRs in parallel each append a row to the **Binding** table of
`docs/adr/README.md` and conflict on it every time; in this round #113, #116, and #117 carried each
other's commits to fix the numbering and the rows, and #121 and #122 conflicted on the rows again.
`scripts/check-docs.sh` already catches a gap or a wrong row, so the cost is the merge, not a wrong
record. Options for the maintainer: a `merge=union` attribute for the file in `.gitattributes`,
vendor-neutral and git's own, with `check-docs.sh` catching a duplicate or a misplaced row a union
leaves; rows in an order a branch does not append to; or nothing, the mechanics as they stand. The
file belongs to the template, so a change may belong upstream.

## Acceptance criteria

- [ ] Given two branches from the same `main` that each add an ADR and its row, when both merge,
  then the second merges without a conflict on the index, and `scripts/check-docs.sh` holds the
  result.

## Read first

- `docs/adr/README.md`, its mechanics; `scripts/check-docs.sh`, checks 1 and 2.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes how the ADR record is kept, which ADR-0007 keeps with the maintainer, and
touches a file of the template.
