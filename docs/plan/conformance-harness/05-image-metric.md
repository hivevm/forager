# Ticket: Compare a rendering with its reference image

## What to build

A comparison of two images for the harness, in a test-support module of the CLI's integration
tests: both decoded to RGBA buffers by the `png` crate, which the module takes as a
dev-dependency, a pixel counted as differing when one of its channels differs by more than a
threshold, and a result that says the share of differing pixels and whether it is within a
tolerance. On a failure it writes the rendering and a difference image for the maintainer. The
encoder's decoding is not changed. Threshold and tolerance are parameters.

## Acceptance criteria

- [ ] Given two equal buffers, when compared, then the share is zero and the case passes.
- [ ] Given one pixel with one channel just over the threshold, when compared, then exactly that
      pixel differs; just at the threshold, none does.
- [ ] Given a share at the tolerance, then the case passes; one pixel above it, then it fails.
- [ ] Given images of different sizes, when compared, then the result is a failure that names
      both sizes, not a panic.
- [ ] Given a palette PNG and an RGB PNG, when decoded, then each compares equal to the RGBA image
      of the same pixels.
- [ ] Given a failing comparison and an output directory, then the rendering and a difference
      image are written there.

## Read first

- ADR-0017 (the `png` crate) and ADR-0012; G-6 and G-12; the spec's *Metric* decision.

## Blocked by

- None (can start immediately)

## Mode

unattended: synthetic buffers decide every criterion; a test-support module and a dev-dependency on
a crate already in the lock file, undone by one revert; nothing ADR-0007 keeps with the maintainer.
It counts against the cap of three open agent branches of [ADR-0007](../../adr/0007-harness.md), and
is dispatched only while that cap allows. Stop when a criterion needs any other new dependency, and
hand off.
