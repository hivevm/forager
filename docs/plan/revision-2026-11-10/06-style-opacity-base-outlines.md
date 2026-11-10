# Ticket: Find why the base layer's outlines of `misc/style_opacity.map` differ, and fix it

## What to build

The three cases of `misc/style_opacity.map` fail since the opacity of styles and layers landed
(5438526), with the reason "the outlines of the base layer differ along the county borders, cause
not yet found" in `crates/cli/tests/conformance/cases.txt`, the one geometry case in scope whose
cause is unknown, and no ticket held it. The opacity they test is not what differs: the base
layer, a polygon layer of `../query/data/bdry_counpy2` with only an `OUTLINECOLOR` and width 1,
differs along the borders drawn from many short segments, the rivers and the lake shore, while its
straight borders match (the difference image the harness leaves under
`target/tmp/msautotest/misc/style_opacity/`).

A hypothesis, not confirmed: MapServer transforms a polygon shape with `msTransformShapeSimplify`
(`mapprimitive.cpp`, the AGG renderer's default `MS_TRANSFORM_SIMPLIFY`), which multiplies by the
inverse cellsize and drops a vertex less than a pixel from its predecessor, and Forager keeps every
vertex. The ticket parked on the branch `parked/tickets-2026-11-07`,
`shape-pixels-by-inverse-cellsize/01`, ports the arithmetic and leaves the dropping out; read it
before porting anything twice.

Read the path at the pin, `msDrawShape` and `msTransformShape` for a polygon layer's outline, find
the cause, and either fix it with a regression test that states the vertices or the pixels, or,
where the fix needs a decision, record the cause in the case's reason and hand off.

## Acceptance criteria

- [ ] Given the three cases, when the harness runs with the suite, then each passes and is flipped
  to `passes` in `cases.txt`, or its reason names the cause found and the ticket that holds it.
- [ ] Given a polygon of many vertices closer together than a pixel, outlined at width 1, when it
  renders through the engine, then its outline is the one MapServer's transformation gives, with a
  test that states the vertices drawn or the pixels, each branch of the ported function the port
  takes over tested.
- [ ] Given every other geometry and text case, when the harness runs, then none that passes
  fails, and the noise of the re-blessed reference images of Forager's own is stated as the
  conventions ask.

## Read first

- `docs/CONVENTIONS.md`, the entries on behaviour ported from MapServer and on re-blessing
  reference images; [ADR-0015](../../adr/0015-agg-rust-rasterizer.md),
  [ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md);
  `docs/semantics/` for the parameters in play; `View` in `crates/symbolize/src/extent.rs` and the
  `pixels` closure of `draw_ops` in `crates/symbolize/src/draw.rs`; MapServer at the pin:
  `msDrawShape` (`src/mapdraw.c`), `msTransformShape` and `msTransformShapeSimplify`
  (`src/mapprimitive.cpp`).

## Blocked by

- None (can start immediately)

## Mode

unattended: the harness decides the cases, a test at the engine decides the port, and the scope is
the transformation of a shape to pixels; a wrong result costs one revert and shows in the
conformance outcome of the other cases. It touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three. Stop: when the cause needs a field of the style IR, such as a `PROCESSING` key, or moves a
case that passes today, or is not found within one session, the session writes what it read and
measured into this file and hands off.
