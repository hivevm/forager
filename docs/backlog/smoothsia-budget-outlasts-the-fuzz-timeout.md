# A layer's nested smoothsia within the visit budget runs past the fuzz run's 10 s

**Observed:** ticket backlog-deviations/03 (2026-11-19). The first 300-second run of the render
fuzz target from `fuzz/seeds/render/`, after the ticket's decoding change, stopped about 46,000
runs in on a timeout: `libFuzzer: timeout after 12 seconds`, past the 10 s limit of Q-3. Its input
decoded to two layers, the polygon and the line layer, sharing a nested `smoothsia` tree on a
polygon of rings of 2, 0, and 7 points and on a multipoint; it took 16.7 s alone on the fuzz build
and about 0.3 s in a release build without the fuzz instrumentation, so nothing loops: the work is
bounded, and the bound is what the instrumented build cannot finish in 10 s. Its bytes are in
commit 20a79cba. The review of ticket 03 changed the decoding, and on it those bytes decode to the
two layers on the polygon alone and take some 11.1 to 11.6 s.

A 300-second run from the seeds on the reworked decoding timed out again, about 40,000 runs in,
on the input below. One layer is enough: it decodes to no shapes, and to the layer `multipoints`,
a line layer of the multipoint of two parts, of 6 points and 1, under the layer `GEOMTRANSFORM`
`smoothsia(smoothsia(simplifypt([shape], [map_cellsize] / NaN), [map_cellsize] / NaN,
[map_cellsize] / NaN, all), [map_cellsize] / NaN, [map_cellsize] / 8.3e-316)`, which the polygon
and line layers share but draw on nothing. The map has no `MAXFEATURES`; the polygon and line
layers carry a class style of `labelcenter`, the label a style of `labelpoly`. Replayed alone on
the fuzz build (`fuzz/target/x86_64-unknown-linux-gnu/release/render <file>`), it takes 12.7 to
13.7 s; the layer `multipoints` drawn alone in a release build takes about 0.54 s, and ends with
`layer multipoints cannot draw 1 multipoint geometry in its render mode`: the transform succeeds,
within its budget, and leaves a multipoint a line layer does not draw. The time lies in
`smooth_sia` and `preprocess` of `crates/symbolize/src/layer_transform.rs`, which charge the
layer's `Budget`, up to `MAX_TRANSFORM_VISITS`, 100,000,000 visits. How many visits the input
charges was not measured, nor which call spends most.

The polygon's path is older than ticket 03; the multipoint's is new since ticket
backlog-deviations/02 let a multipoint reach a layer's transform, and the render target builds one
since ticket 03. A probe run of ticket 03, which draws each input up to three times, timed out the
same way on another input of two layers' `smoothsia` (8.9 s alone on the fuzz build).

**Input** (430 bytes, base64; `bash scripts/fuzz.sh --replay render <file>` replays it on the
target of ticket backlog-deviations/03 as its review left it, and fails with `libFuzzer: timeout
after 11 seconds`):

```text
uBwAVAAAAAAAAEAAAAAAAAAAJQAAAAwAVAAAAAAAAEAA////////////////////////////////////////////AAAAAAAAJQAA
AAAAAAAAkIeSlp5mZa/SkWluZTojOiAiAEZlYWF0dXR1cmVDb2xsZWN0h4eHh4eHh4eH/////////yItZv//////////////////
////////////////////////////////////////////h4eHh4eHj4d+h4eHhwwAVAAAAAAAAED//////////////2VDb2xsZWN0
aW9uIjBdXV19fQpdfQrv7+/v71RVVVVVVVUF7+/v7/X1IQtZyEIWsgDv7+/v9f//AQGBAAAAra2t30VORUnU+oV/f3/19f//////
/////////////////////////////////////////////////////////////////wAAAAD/////////////CQAAAAAAAAD/////
////////////////////////////////////////////////////AAAAAAAAACUAAAAAAAAAAA==
```

**Where:** `smooth_sia`, `smooth_part`, and `preprocess` in
`crates/symbolize/src/layer_transform.rs`, its `MAX_TRANSFORM_VISITS` and `Budget`; the limits of
`scripts/fuzz.sh` (`-timeout=10`); Q-3 and Q-9 of `docs/SPECIFICATION.md`.

**What a change weighs:**

- **Measure first:** the visits the input charges, per layer and per call, and what one visit
  costs in `smooth_part`, whose window walk does more per point than the charge counts. If the
  charge undercounts, charging what the window walk does closes it within the bound as it stands.
- **A lower bound:** fewer visits per layer cut every transform's worst case, including the
  product's, and may leave undrawn a real map MapServer smooths; Q-9 sets the bound for the
  product, not for the fuzz build.
- **A bound per render rather than per layer:** layers of the same tree add up, as the first input's
  two did; a bound the layers share cuts that, at the cost of a layer drawn or not by what the
  layers before it spent, but not the input above, whose one layer is past 10 s alone.
- **Leave the product as it is and keep the fuzz build in time:** fewer transformed layers in the
  render target leave the input above where it is; and Q-3's 10 s is measured on the fuzz build.
