# Ticket: The browser renders a mapfile and GeoJSON in an example page

## What to build

The WebAssembly bindings expose one function that takes mapfile text, GeoJSON per layer name, and
a viewport, and returns the RGBA image and the diagnostics. An HTML page under `examples/web/`
has a textarea for the mapfile, one for GeoJSON, and a canvas; a small Rust server serves it
locally with one command. The WASM build runs in `scripts/check-rust.sh` with the size check of
Q-5.

## Acceptance criteria

- [ ] Given the bindings built for `wasm32-unknown-unknown` with `wasm-bindgen --target web`, when
      the gzipped `.wasm` is measured, then it is under 3 MB, and the check fails above it (Q-5).
- [ ] Given the function behind the binding called natively with the fixture of ticket 02, then its
      RGBA output equals that ticket's reference image (G-5).
- [ ] Given the example served by the local server, when the page is opened and "Render" pressed
      with the sample mapfile and GeoJSON, then the map appears on the canvas and diagnostics are
      listed (G-5, demo).
- [ ] Given a source reference with a credential property, when it reaches the bindings, then it is
      rejected with a diagnostic (Q-7).

## Read first

- ADR-0008 (the WASM build and size check), ADR-0009 (hosts `bindings-wasm` and `dev-server`);
  the golden path of `docs/ARCHITECTURE.md`.

## Blocked by

- docs/plan/browser-tracer/02-cli-renders-geojson.md

## Mode

with the human — it defines the public API of the browser bindings and the credential filter,
which ADR-0007 keeps with the human.
