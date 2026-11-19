# Ticket: A layer's GEOMTRANSFORM draws on a multipoint

<!-- Copy to docs/plan/<spec>/<NN>-<slug>.md, next to the spec it slices, if it has one. A
     vertical slice: a narrow but complete path through every layer the change touches, demoable
     or verifiable on its own, sized to one fresh agent session. Claim it by creating the branch
     ticket/<spec>/<NN>-<slug> BEFORE any work, so parallel sessions sharing the repository skip
     it. The branch that lands it deletes this file. Decided in
     docs/adr/0005-feature-layer.md. -->

## What to build

A layer with a `GEOMTRANSFORM` draws nothing on a multipoint and warns, because `Shape::of`
refuses one. MapServer transforms such a shape: its GEOS transforms take the first part of a point
shape (`msGEOSShape2Geometry`, `src/mapgeos.c:365-374`), as the port already does for a point.
The port's multipoint keeps a point shape's parts: one for a GeoJSON or a shapefile multipoint,
one per `POINTS` block of an inline feature.

The session maps a multipoint to a point shape of its parts and checks each node of the transform
tree against MapServer's C for what it takes from such a shape. The semantics page drops the
multipoint from what fails a layer's transform. The change deletes
`docs/backlog/layer-transform-on-a-multipoint.md`.

## Acceptance criteria

- [ ] Given a layer with a `GEOMTRANSFORM` of each node of the transform tree, when it draws a
  multipoint of several parts, then it draws what MapServer's C computes from that shape, and no
  warning of a refused multipoint.
- [ ] Given a multipoint of one part, when a layer's transform draws it, then it draws as the same
  point does.
- [ ] Given the msautotest cases of the scope, when they run before and after, then each case that
  moves is named in the hand-off with why.

## Read first

- Touches: `crates/symbolize/src/layer_transform.rs` (`Shape::of`), its tests,
  `docs/semantics/style-symbol.md` (the paragraph on what fails a layer's transform)
- `docs/backlog/layer-transform-on-a-multipoint.md`
- The glossary's *layer transform* and *transform tree*.
- Seam: the unit tests of `layer_transform.rs` that draw a point through each node.

## Blocked by

- None (can start immediately)

## Mode

unattended: unit tests decide the criteria and the msautotest run shows any moved case, a wrong
result costs one revert, and it touches no clause the harness ADR keeps with the human. Stop when a
node's C takes something from a multipoint that the point shape cannot express, or when a case of
the scope moves to a worse result; hand off with the case and the C it read.
