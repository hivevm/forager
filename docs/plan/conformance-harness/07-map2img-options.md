# Ticket: Apply map2img's -l, -e, and -s to a case

## What to build

The harness applies the `map2img` options of a case's `RUN_PARMS` line to the style IR before
it renders, as `map2img` applies them: `-l` turns the named layers on, `-e` sets the extent, `-s`
sets the image size. The cases that need only these options move from out of scope into the list.

## Acceptance criteria

- [ ] Given a line with `-l "a b"`, when the case is rendered, then layers `a` and `b` are drawn
      as `map2img` draws them, as its source reads.
- [ ] Given a line with `-e` and one with `-s`, when the cases are rendered, then the extent and
      the size are those of the line.
- [ ] Given a line with any other option, then the case stays out of scope with that reason.
- [ ] Given the list after the change, then every case it moves into scope holds its outcome.

## Read first

- `map2img`'s option handling in the MapServer source at the pin; the spec's *Case* decision.
- The harness of ticket 06 is the seam.

## Blocked by

- docs/plan/conformance-harness/06-geometry-cases.md

## Mode

unattended: the harness decides every criterion; the harness alone, undone by one revert; nothing
ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches of
[ADR-0007](../../adr/0007-harness.md), and is dispatched only while that cap allows. Stop when an
option's meaning needs a change of the style IR, and hand off.
