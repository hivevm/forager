# Three layers of one nested smoothsia, each within the visit budget, sit at the fuzz run's 10 s

**Observed:** ticket backlog-deviations/06 (2026-11-19). After 06's fix, which made the passes of
`smoothsia` reuse one set of buffers, the first 300-second run of the render fuzz target from
`fuzz/seeds/render/` stopped about 4,800 runs in on a timeout: `libFuzzer: timeout after 12
seconds`. Its input decodes to the three layers that share the target's `GEOMTRANSFORM`, the
polygon, the line, and the multipoint layer, under four `smoothsia` nested, every number
`[map_cellsize] / NaN` but the outermost iterations, `[map_cellsize] / 7.44748961190531e223`: the
inner three with the preprocessing `all`, the outer with `angle`. Each layer is charged within
`MAX_TRANSFORM_VISITS`, and none is past it:

| Layer | Its shape | Passes of the outer smoothsia | Visits charged |
| --- | --- | --- | --- |
| `multipoints` | a part of 5 points, 31 after the preprocessing | 168,037 of 93 | 15,627,594 |
| `polygons` | rings of 9, 0, 2, and 1 points, 141, 0, 2, and 1 after it | 168,037 of 432 | 72,592,632 |
| `lines` | the same rings | 168,037 of 432 | 72,592,632 |

Together, 160,812,858 visits in one render. Drawn in a release build, the render takes about
0.87 s, 5.4 ns a visit, of which the outer passes take 0.39 s for each of the two 72-million
layers. The charge counts the work: a pass of the polygon's is charged 144 points by a window of
3, 432, and reads 139 windows of 3 and keeps 2 ends; the parts of fewer than 3 points are charged
and skipped. So nothing undercounts and nothing loops.

On the fuzz build the input sits at the 10 s limit, over or under it with the machine's load (1
minute load average, 24 cores, other sessions running):

| Build | Load | Time |
| --- | --- | --- |
| 06's fix | about 17 | 8.7 s, 9.8 s |
| 06's fix | 32 | 7.9 s |
| 06's fix | 44 | 6.2 s |
| 06's fix | 41 to 61 | 13.5 s of CPU, or the timeout |
| 06's fix | about twice the cores | 15.2 to 16.0 s, about 95 ns a visit |
| before 06's fix, beside it | the same | 14.0 to 15.1 s |

The first four rows are the review's of 06, on builds of its own; the last two are 06's. A pass
over 141 points spent little on the allocations 06 removed. The fuzz build's times move by up to
2.5 times with the load: 06's own input took 5.0 s and 12.7 s on the same build minutes apart.

**Decided:** the maintainer keeps this entry in the backlog, to be decided in the next design
revision (2026-11-19).

**Input** (750 bytes, base64; `bash scripts/fuzz.sh --replay render <file>` replays it):

```text
eyJ0eaiSOnjv7yB7Im71UGUnOnNsbWVjdAAGbAptb2IKCgoKCgoK7+/v7+9UVVVVVVUsICJmZWF0dWIKCgoKCuvr6+vr6+vr6wMA
AAAAAAAA6+vr6+vr6+vr6+vr6+vr6+vr6+vr63l5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5
eXl5eXl5eXl5nXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl56+vr6+vr6+vrATf/////////////////
//////////////9B/wkA////////Onjv7yB7Im71UGUnOnNsbWVjdGklb24lJSUlJSUlIiy4DACs+gAA+/v7+/v7QPv7+/v7+/v7
+/v7+/v7+/sAAAAAAPv7+/v7+/sBAPv7+/v7+/v7+/v7+/v7AAAAQAD/////////////////////////////////////////////
/////////////////////////////////////wAAAAAAOmUiOiBwb2ludCBl///////////////////////3////////////////
/8dpbmUiOt4iK0ZlYXR1AAAAAAAAAAAAAAAAAAg6AAAA27//AAAAAAAAjgAAAJKcj9L/////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////wGluZSI63iIrRmVhdHVyZQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAENvbGxlY3SH
h4f4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+IeHh4eHh4cgIodw
```

**Where:** `smooth_sia` of `crates/symbolize/src/layer_transform.rs`, its `MAX_TRANSFORM_VISITS`
and `Budget`, which a layer takes anew in `transformed` of `crates/engine/src/lib.rs`; the
render target's three layers of one `GEOMTRANSFORM` in `fuzz/fuzz_targets/render_input.rs`; the
limits of `scripts/fuzz.sh` (`-timeout=10`); Q-3 and Q-9 of `docs/SPECIFICATION.md`. ADR-0006
keeps the limits with the maintainer.

**What a change weighs:**

- **A bound per render rather than per layer, the maintainer's (ADR-0006):** the three layers
  here charge 161 million visits together, and one bound of 100 million for the render would stop
  them at about two thirds of this input's work. A layer would then be drawn or not by what the
  layers before it spent. Q-9 names the bound per layer.
- **A lower bound per layer, the maintainer's:** about 30 million visits a layer would keep three
  such layers under 10 s on the fuzz build at 95 ns a visit. It cuts the product's worst case too,
  and may leave undrawn a real map MapServer smooths. Q-9 sets the bound for the product, not for
  the fuzz build.
- **Fewer layers of one transform in the render target:** one transformed layer, or a tree for
  each, keeps a render to one layer's bound, about 7 s for this input's heaviest layer; the
  product stays as it is, and Q-3's 10 s is measured on the fuzz build.
- **A cheaper window walk, within a session's Mode:** the fuzz build's sanitizer and coverage make
  a visit some 18 times dearer than in a release build. 06 removed a pass's allocations, which
  this input's long parts barely paid. The walk's per-neighbour `checked_sub` and `filter` options
  and its indexed reads through the window's indices are its cost now, and how much a cheaper walk
  saves was not measured. It changes no limit and no bound's scope, and may bring this input under
  10 s at the loads above.
