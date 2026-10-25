# Ticket: Measure MapServer's noise and hold G-6

## What to build

A CI job runs the pinned MapServer over the in-scope cases and writes its renderings into a
directory; the harness compares that directory with the references by the same metric and
reports, per case, the share of differing pixels at each threshold of a small set. The largest
share over the category, with a margin the maintainer picks, sets the threshold and the tolerance
of the category geometry and symbols in G-6, as a drafted specification change. Once signed, the
harness takes the numbers and cites G-6. How MapServer runs on CI, built from the tag or from a
pinned image, is decided first, through `propose-adr`, in its own pull request.

## Acceptance criteria

- [ ] Given the proposed ADR, when the CI job runs, then MapServer runs as it decides and nowhere
      inside the dev container.
- [ ] Given the job's output, when the harness compares it, then it reports per case the share at
      each threshold, and the maximum over the category.
- [ ] Given a case MapServer itself does not reproduce within any threshold of the set, then the
      report names it, and the scope list moves it out of scope with that reason.
- [ ] Given the signed specification change, then the harness and its list use its threshold and
      tolerance, and the harness cites G-6 with a `Verifies:` marker.

## Read first

- The Assumption on the reproducibility of the references, G-6, and the Constraint on the pin, in
  the specification; ADR-0004 and ADR-0010 (no containers in the dev container); ADR-0011 and
  ADR-0012 (the supply chain of CI).
- MapServer's own comparison in its test library: an exact checksum first, then perceptualdiff.

## Blocked by

- docs/plan/conformance-harness/06-geometry-cases.md

## Mode

with the human: it decides an ADR and drafts a change to the specification, both the maintainer's
([ADR-0007](../../adr/0007-harness.md)).
