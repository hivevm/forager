# Ticket: G-14 gets a browser test

## What to build

A test that opens the browser example in a headless browser, with the bundle served, and checks that
the page shows the image the bindings return for its sample mapfile and GeoJSON, and lists their
diagnostics. G-14 then has its `Verifies: G-14` marker, and the traceability check lists no pending
criterion.

`propose-adr` runs first, because the browser and its driver are tools and dependencies no ADR
decides. ADR-0034 left the browser test out of scope. The maintainer named two candidates,
headless Chromium driven by wasm-bindgen-test or by Playwright, and the ADR compares those two
first. It may weigh others beside them:

- **Headless Chromium from a script.** Chrome is preinstalled on GitHub's Ubuntu runner. A bash
  check could serve the page with `dev-server` and run `--headless --screenshot` or `--dump-dom`.
  This is the simplest. It reads the page only after a time budget, which may be flaky.
- **A WebDriver client in a Rust test, such as fantoccini, against chromedriver.** It can wait for
  the canvas and the list. It adds crates (ADR-0011, Q-6) and a driver binary (ADR-0007).
- **wasm-bindgen-test in a headless browser.** It runs Rust tests inside the browser but does not
  open `examples/web/index.html`. So it tests the bindings, not the example G-14 names.
- **Playwright.** It needs Node. ADR-0009 keeps Node out of the Dev Container, and ADR-0034 rejected
  Node for the same reason.

The ADR also weighs where the test runs. The Dev Container has no browser (ADR-0009). Possible
answers: a CI job only, a browser added to the container under ADR-0009, or a test that skips
without a browser. A skip must not pass in CI. The ADR also weighs how the test reads the image: the
canvas pixels compared with what the bindings return for the same input, or a weaker check that the
canvas is non-empty. It names Q-5 (no change to the bundle) and ADR-0035 (the page loads only
vendored files).

## Acceptance criteria

- [ ] Given the ADR of the browser test, when it is proposed, then it stands in a commit of ADRs
  only, handed to the maintainer before anything is built against it.
- [ ] Given `examples/web/` and the bundle `scripts/build-web.sh` builds, served as `dev-server`
  serves them, when the test opens the page in a headless browser, then the canvas holds the image
  the bindings return for the sample mapfile and GeoJSON at the page's viewport.
- [ ] Given the same page, when the bindings return diagnostics, then each is listed under the
  image. If the sample mapfile loads cleanly, the ADR chooses between a sample that gives a
  diagnostic and a second input; only the sample's own path cites G-14.
- [ ] Given the test, when `scripts/check-traceability.sh` runs, then G-14 is cited and no
  criterion is pending.
- [ ] Given CI, when the test fails or finds no browser, then the job fails.

## Read first

- Touches: a test under `crates/dev-server/` or a check under `scripts/`, as the ADR decides;
  `.github/workflows/checks.yml`; `scripts/check-all.sh`; possibly `.devcontainer/` and
  `scripts/install-tools.sh`
- `examples/web/index.html` (the canvas `#map`, the list `#diagnostics`, the import of
  `./pkg/forager_bindings_wasm.js`), `crates/dev-server/src/main.rs`, `scripts/build-web.sh`
- `docs/ARCHITECTURE.md` (**`bindings-wasm`**, **`dev-server`**)
- `docs/adr/0034-wasm-renders-compared-in-wasmi.md` (*Context*, *Out of scope*, the Node
  alternative), `docs/adr/0009-dev-container.md` (what it leaves out),
  `docs/adr/0007-rust-toolchain.md`, `docs/adr/0011-crate-supply-chain.md`,
  `docs/adr/0035-vendored-third-party-files.md`
- Skill: `propose-adr`
- Seam: the page as a browser shows it

## Blocked by

- None (can start immediately)

## Mode

with the human: it starts with an ADR, which the maintainer decides (ADR-0006). The session stops
after the ADR's commit and the hand-off. It builds the test only after the maintainer has reviewed
the ADR.
