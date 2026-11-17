# A line buffered by a distance near i_overlay's grid step asks for gigabytes, or vanishes

**Observed:** revision-2026-11-16 ticket 18 (2026-11-17), by probes run against `buffer` in
`crates/symbolize/src/geometry.rs` before that ticket's change and after it. That change does not
reach these shapes, since their size is far above 2^-450. In a release build:

- the line `(0, 0) (1, 0)` buffered by `1e-8` aborts with
  `memory allocation of 4026531840 bytes failed` under a 4 GB address-space limit, and is killed
  without one. The debug build of `cargo test` behaves the same way;
- the same line buffered by `1e-9` or `3e-9` returns no polygon, so a positive distance makes it
  vanish. By `3e-8` and above it returns one polygon;
- the unit square buffered by the same distances keeps a polygon each time, and asks for no such
  memory.

i_overlay snaps the shape to a grid of about 2^-30 of its size, some `9.3e-10` here. A line's
stroke whose width is a few grid steps lies at this limit. Neither the cause in i_overlay's stroke
builder nor the cost the charges of Q-9 would have to bound is found yet. The charges of `buffer`
do not stop it: they bound the points of the arcs, not this allocation. An allocation that fails
aborts the process and is no panic, so the render fuzz target would report it as a crash, and a
host would lose its process.

**Where:** `buffer` in `crates/symbolize/src/geometry.rs`, the stroke of a line in i_overlay 4.5.2
(`src/mesh/stroke/`), which geo's `buffer_with_style` calls.

**What a change weighs:**

- **Finding the allocation in i_overlay's stroke first:** a probe under a debugger or a heap
  profiler names it. Its size may follow from a count i_overlay computes from the distance and the
  grid, which `buffer` could bound before it calls geo.
- **Refusing a line's buffer narrower than some multiple of the grid step:** a few lines, but
  GEOS keeps such a buffer, so a refusal is a deviation to write down in `docs/semantics/`.
- **Reporting it to i_overlay:** the defect may be fixed upstream, in the i_overlay 9 that geo is to
  take up (ADR-0042).
