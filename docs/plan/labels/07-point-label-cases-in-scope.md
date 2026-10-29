# Ticket: The point-label cases come into the harness's scope

## What to build

The conformance harness renders the cases whose labels are all of point features, in TrueType fonts
and Latin script, under the category text, compared by the metric with provisional numbers, each
listed as passing or failing. Every other label case names its reason more precisely: labels on
lines, labels in polygons, a bitmap font, complex script.

## Acceptance criteria

- [ ] Given the list after the change, then every point-label case of `misc` and `renderers` is in
      scope under the category text, and every other label case has its precise reason.
- [ ] Given the harness, then every case it moves into scope holds its listed outcome, and the
      category text is compared with its own provisional threshold and tolerance, citing nothing.
- [ ] Given the noise measurement, then it runs over the text cases as over geometry and symbols.

## Read first

- The conformance harness and its list; ADR-0027; the spec of the conformance harness, *Scope* and
  *Tolerance*.

## Blocked by

- docs/plan/labels/06-label-cache.md

## Mode

unattended: the harness decides every criterion; the harness alone, undone by one revert; nothing
ADR-0007 keeps with the maintainer. It counts against the cap of ADR-0007. Stop when a case's
category is unclear from its mapfile, and hand off with the list of such cases.
