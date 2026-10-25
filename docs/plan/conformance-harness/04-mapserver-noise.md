# Ticket: Measure MapServer's noise against its own references

## What to build

A CI job runs the pinned MapServer over the cases of the scope list and compares each of its
renderings with the reference by the metric of ticket 03. The largest channel difference and share
it shows set the threshold and the tolerance of the category geometry and symbols, with a margin
the maintainer picks. The numbers go into G-6 as a drafted specification change. How MapServer
runs on CI, built from the tag or from a pinned image, is decided first, through `propose-adr`.

## Acceptance criteria

- [ ] Given a `proposed` ADR on running the pinned MapServer on CI, when the maintainer accepts
      it, then the job runs it that way and nowhere inside the dev container.
- [ ] Given the cases in scope, when the job runs, then it reports per case the share of differing
      pixels at each threshold of a small set, and the maximum over the category.
- [ ] Given that report, when the maintainer signs the drafted change, then G-6 states the metric,
      the threshold, and the tolerance of the category geometry and symbols.
- [ ] Given a case MapServer itself does not reproduce, then it is named in the report and
      proposed for the scope list's out-of-scope entries with that reason.

## Read first

- The Assumption on the reproducibility of the references, G-6, and the Constraint on the pin,
  in the specification; ADR-0004 and ADR-0010 (no containers in the dev container); ADR-0011 and
  ADR-0012 (the supply chain of CI).
- MapServer's own comparison in its test library: an exact checksum first, then perceptualdiff.

## Blocked by

- docs/plan/conformance-harness/02-fetch-data-and-references.md
- docs/plan/conformance-harness/03-image-metric.md
- docs/plan/conformance-harness/05-geometry-cases.md (its scope list)

## Mode

with the human: it decides an ADR and drafts a change to the specification, both the maintainer's
([ADR-0007](../../adr/0007-harness.md)).
