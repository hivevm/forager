# ADR-0021: The browser example loads third-party JavaScript only from copies vendored in the tree and checked by checksum

- **Status:** 🟢 accepted
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `examples/web/`, its `vendor/` directory, and every JavaScript library the example loads

## Context

The browser example displays the image the WASM bindings render (G-5) on a fixed canvas: one
extent, one size, no panning or zooming. Forager's purpose is to replace server-rendered tiles in
the browser, and a slippy map shows that the bindings answer a viewport per tile, which the fixed
canvas does not. `render(mapfile, sources, width, height, extent)` already takes the extent of a
tile, so a map library can ask Forager for each tile it needs, with no server in between.

A map library is the example's first JavaScript dependency. Crate dependencies are governed by
[ADR-0012](0012-crate-supply-chain.md); nothing governs JavaScript. The example runs in the
user's browser, and the project gives security precedence over convenience: a library loaded from
a third-party origin at page load is code the repository neither reviews nor pins. The licence
must be permissive (Q-6). The library stays out of the WASM bundle and its size limit (Q-5), and
out of the core (Q-1).

The sample data is in plain map units (0..100 × 0..60), not geographic coordinates, and Forager
does not reproject geometry yet, so the example uses a planar coordinate reference system without
a base map.

## Decision

We will load every third-party script and stylesheet of the browser example from a copy under
`examples/web/vendor/`, byte for byte as its release published it and with its licence, whose
SHA-256 checksums a check compares against the checked-in `examples/web/vendor/SHA256SUMS` on
every run; the first such copy is Leaflet 1.9.4 (`dist/leaflet.js`, `dist/leaflet.css`, and
`LICENSE`, under `examples/web/vendor/leaflet/`), which draws the example's tile view.

Replacing or updating a vendored library, Leaflet among them, follows this decision: a new copy and
new checksums, with the library's licence permissive (Q-6).

**Out of scope:** a JavaScript package manager or build step for the example; a base map from a
tile server, which waits for reprojection and geographic sample data; Leaflet in the published
bindings, which stay library-agnostic.

## Alternatives considered

- **Leaflet from a CDN with Subresource Integrity** — no files in the tree, and the integrity
  attribute pins the content; but every page load contacts a third-party origin, and the example
  stops working offline and behind a proxy.
- **npm with a lock file** — the usual route for JavaScript, with `npm audit`; a toolchain and a
  `node_modules` tree for two static files, against Simplicity.
- **OpenLayers** — richer, with first-class projections; an ES module tree several times Leaflet's
  size that wants a bundler.
- **MapLibre GL JS** — vector-tile renderer with WebGL; it would draw the map itself instead of
  showing Forager's raster tiles, which is what the example exists to show.
- **A pan-and-zoom of our own on the canvas** — no dependency, but tiling, input handling, and
  zoom levels written again for an example.
- **Leaflet 2.0** — published only as `2.0.0-alpha.1`; an alpha in an example readers copy from.

## Sources / Prior art

- Leaflet 1.9.4 on npm, <https://www.npmjs.com/package/leaflet>, checked 2026-10-24: `latest`
  dist-tag, BSD-2-Clause, published 2023-05-18; tarball integrity
  `sha512-nxS1ynzJOmOlHp+iL3FyWqK89GtNL8U8rvlMOsQdTTssxZwCXh8N2NB3GDQOL+YR3XnWyZAxwQixURb+FA74PA==`
  verified against the downloaded file.
- Leaflet `GridLayer.createTile` and `CRS.Simple`, <https://leafletjs.com/reference.html>.
- Subresource Integrity, <https://www.w3.org/TR/SRI/>.

## Consequences

- Positive: the example pans and zooms, and each tile is a call of `render` with the tile's
  extent, the use Forager is built for. No page load reaches a third-party origin, and a changed
  vendored file fails the checks.
- Negative / trade-offs: updating Leaflet is a manual copy with new checksums, and no advisory
  database watches the copy; the maintainer follows Leaflet's releases. The tree carries about
  160 KB of minified JavaScript and CSS.
- Follow-ups: a base map once reprojection lands; Leaflet 2.0 once it is released as stable.

## Enforcement

`scripts/check-vendor.sh` runs `sha256sum --check` on `examples/web/vendor/SHA256SUMS`, which
lists every file under `examples/web/vendor/`, and fails on a mismatch or a file not listed;
`scripts/check-all.sh` and CI run it, and its self-test cites this ADR with `Verifies:`. The same
script fails when a `<script>` or `<link>` of `examples/web/index.html`, or an `import` of its
module script, names another origin (`http:`, `https:`, or `//`). A `url()` in the page's inline
style is not read; a reviewer holds that.
