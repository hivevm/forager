# Ticket: Compare a rendering with its reference image

## What to build

A comparison of two images for the harness: both decoded to RGBA buffers of the same size, a
pixel counted as differing when one of its channels differs by more than a threshold, and a
result that says the share of differing pixels and whether it is within a tolerance. On a failure
it writes the rendering and a difference image beside each other for the maintainer. Threshold
and tolerance are parameters; their numbers come from ticket 04.

## Acceptance criteria

- [ ] Given two equal buffers, when compared, then the share is zero and the case passes.
- [ ] Given one pixel with one channel just over the threshold, when compared, then exactly that
      pixel differs; just at the threshold, none does.
- [ ] Given a share at the tolerance, then the case passes; one pixel above it, then it fails.
- [ ] Given images of different sizes, when compared, then the result is a failure that names
      both sizes, not a panic.
- [ ] Given a palette or RGB PNG reference, when decoded, then it compares equal to the RGBA image
      of the same pixels.
- [ ] Given a failing comparison and an output directory, then the rendering and a difference
      image are written there.

## Read first

- The encode crate and ADR-0017 (the PNG crate; decoding reuses it).
- G-6 and G-12 in the specification; the spec's *Metric* decision.
- The seam is a test-support module of the CLI's integration tests, next to the G-1 test.

## Blocked by

- None (can start immediately)

## Mode

unattended: synthetic buffers decide every criterion; the change is a test-support module, undone
by one revert; it touches nothing ADR-0007 keeps with the maintainer. Stop when a criterion needs a
new dependency beyond the `png` crate, and hand off.
