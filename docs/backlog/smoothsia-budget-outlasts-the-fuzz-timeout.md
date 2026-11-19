# Two layers' nested smoothsia within the visit budget run past the fuzz run's 10 s

**Observed:** ticket backlog-deviations/03 (2026-11-19). The first 300-second run of the render
fuzz target from `fuzz/seeds/render/`, after the ticket's decoding change, stopped about 46,000
runs in on a timeout: `libFuzzer: timeout after 12 seconds`, past the 10 s limit of Q-3. Replayed
alone on the fuzz build (`fuzz/target/x86_64-unknown-linux-gnu/release/render <file>`), the input
takes 16.7 s. The same map, decoded through `render_input.rs` and drawn in a release build without
the fuzz instrumentation, takes about 0.3 s, so nothing loops: the work is bounded, and the bound
is what the instrumented build cannot finish in 10 s.

What the input decodes to: the polygon and line layers share a layer `GEOMTRANSFORM` of
`smoothsia(smoothsia([shape], [map_cellsize] % 7.5e25, [map_cellsize] ^ -1.5e306, all),
[map_cellsize] / -1.4e265, [map_cellsize] / 2.0e243)`, and the features are a polygon whose
first ring has two points and second none, at coordinates up to 1e260, and a multipoint of two
parts, four points and none. Drawn alone in the release build, each layer spends about 108 ms
on the polygon and 44 ms on the multipoint; no other layer takes more than 20 ms. The time lies in
`smooth_sia` and `preprocess` of `crates/symbolize/src/layer_transform.rs`, which charge the
layer's `Budget`: up to `MAX_TRANSFORM_VISITS`, 100,000,000 visits, for each of the two layers.
How many visits the input charges was not measured, nor which of the two calls spends most.

The polygon's path is older than the ticket; the multipoint's share is new since ticket
backlog-deviations/02 let a multipoint reach a layer's transform, and the render target builds one
since ticket 03. A probe run of ticket 03, which draws each input up to three times, timed out the
same way on another input of two layers' `smoothsia` (8.9 s alone on the fuzz build).

**Input** (754 bytes, base64; `bash scripts/fuzz.sh --replay render <file>` replays it on the
target of ticket backlog-deviations/03):

```text
eyJpbmUiOiAiRmVhdHVyZUNvbGxlSdR7InR5cGUiOgAiAQGBACAiRmVhdHUAARubpKf////3//////////////8HBwcHSQXv7+/v
7+/v7+/v7+9w72nv73iPYXDv7+/v7+/vqIs6eO/vaHsibvX19fX19fUAARukCQAAcG9p7+/v7/X//wEBgQAA7e3t7e3t7e3tgQAg
IkZlYXR1AAEbm6Sn////9///////////////BwcHB0kF7+/v7/X//wEBgQAA7e3t7e3t7e1hdHUAARsA9fX1JPX19fUR9WdseXBo
LXBvaW50GBgYGBgYGBglIiwrInBlb21lAAAAAAAAAAZsCm1vYgoKCgoKCgrv7+/v71RVVVVXY2lyY2xlc/X19e9F9U5FSQXv7+/v
9f//AQAAIAEBgf////////////////////r6+vr6+vr6+vr/B/////8AJm9uItvf29vb29vb29vb29vb29vb29vb29vb29sYGBgY
GBgYGBgYGBgYGBgYGBgYGBgCjjjjjjjjjhgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBglIiwrInBlb21lAAAAAAAAAAZsCm1vYgoK
CgoKCgrv7+/v71RVVVVXY2lyY2xlc/X19e9F9U5FSQXv7+/v9f//AQGB////////////////////+vr6+vr6+vr6+v9ORUkF7+/v
7/X//wEBgf////////////////////r6+vr69vr6+vr///////9ycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJy
cnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJycnJyACZvbiIsICJwXV1dfT4A
An0KXX0AAK2t2wrb29vb29vb29vb29vb29vb29vb29vb39vb29vb25ukpKen29vb29vb29vb29vb29vb29vb29vf29vbf9vb29vb
29vb2w==
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
- **A bound per render rather than per layer:** two layers of the same tree double the cost here;
  a bound the layers share halves it, at the cost of a layer drawn or not by what the layers
  before it spent.
- **Leave the product as it is and keep the fuzz build in time:** the render target drawing one
  transformed layer, not two, halves the case, but leaves a deeper tree or a slower host where it
  is; and Q-3's 10 s is measured on the fuzz build.
