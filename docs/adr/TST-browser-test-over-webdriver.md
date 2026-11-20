# TST-browser-test-over-webdriver: The browser test of G-14 drives headless Chrome over WebDriver with a client of its own, compares the canvas with the native render, and runs in a CI job of its own

- **Status:** 🟢 accepted
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the browser test `crates/dev-server/tests/browser.rs` and its WebDriver client, the dev-dependencies of `crates/dev-server` and their check in `crates/arch-test/tests/dependencies.rs`, the script `scripts/check-browser.sh` that runs the test, the job of the browser test in `.github/workflows/checks.yml` and the line of `scripts/check-all.sh` that run that script, and the variable `FORAGER_BROWSER_TEST` that job sets

## Context

G-14 asks that the HTML example, opened in a browser with the bundle served, displays the image
the bindings return for its sample mapfile and GeoJSON and lists their diagnostics. No test
verifies it, so the traceability check lists it as pending.
[TST-wasm-renders-in-wasmi](TST-wasm-renders-in-wasmi.md)
left the browser test out of scope. It holds the wasm32 build of the engine bit-identical to the
native one (G-9), but not the page, its JavaScript, or the glue wasm-bindgen generates.

The forces:

- **The page is what G-14 names.** `examples/web/index.html` imports
  `./pkg/forager_bindings_wasm.js`, loads its sample and the files beside it, draws the result on
  the canvas `#map`, and lists the diagnostics in `#diagnostics`. `crates/dev-server` serves the
  page and the bundle `scripts/build-web.sh` writes. A test that does not open this page leaves
  G-14 open.
- **No browser in the Dev Container.** [OPS-dev-container-adds-rust](OPS-dev-container-adds-rust.md)
  installs neither a browser nor Node. GitHub's Ubuntu runner ships Google Chrome with a
  ChromeDriver of the same version (image 20261111: both 157.0.8103.50), and `CHROMEWEBDRIVER` names
  the driver's directory.
- **Supply chain.** Every crate passes `cargo deny`
  ([SEC-crates-locked-and-denied](SEC-crates-locked-and-denied.md)). On a trial, fantoccini 0.22.1
  without TLS, with tokio, passed `cargo deny check` but added 81 packages to `Cargo.lock` (hyper,
  tokio, the icu and idna crates, url, time). WebDriver is HTTP with JSON bodies. The dev server
  already speaks HTTP with `std` alone, and `serde_json` is in the workspace.
- **What the page draws.** The sample renders without diagnostics (the test
  `the_browser_example_renders_without_diagnostics` of `crates/bindings-wasm`). A list of
  diagnostics needs an input that has some.
- **A dev edge between two hosts.** The native reference render comes from `crates/bindings-wasm`, a
  host like the dev server.
  [ARCH-crates-as-ports-and-adapters](ARCH-crates-as-ports-and-adapters.md) lets a host depend on
  the crates above the hosts, and its test in `crates/arch-test/tests/dependencies.rs` holds the
  normal edges only. A dev-dependency of the dev server's tests on the bindings ships nothing and
  leaves that table as it stands. It is the first edge from host to host, so this ADR names it.
- The bundle and its JavaScript API stay unchanged (Q-5). The page loads only vendored files
  ([SEC-third-party-files-vendored](SEC-third-party-files-vendored.md)), and the test changes
  neither.

## Decision

We will verify G-14 with a Rust test in `crates/dev-server/tests/browser.rs`. It starts the dev
server and drives headless Chrome through ChromeDriver over the W3C WebDriver protocol. Its client
is the project's own, written on `std` and `serde_json`. It compares the pixels of the canvas with
the native render of the same input. It runs in a CI job of its own that requires it.

What binds:

- **The page as shipped.** The test opens `examples/web/index.html` from the dev server binary
  Cargo builds for the test (`CARGO_BIN_EXE_forager-dev-server`), with the bundle
  `scripts/build-web.sh` wrote, at a free port. It changes no file of the page.
- **The client.** It makes the few WebDriver calls the test needs: a session with headless Chrome,
  navigation, running a script, and ending the session. It adds no crate to `Cargo.lock`: the dev
  server's dev-dependencies are `forager-bindings-wasm` and `serde_json` alone. Another one is a
  revision of this ADR.
- **The image.** The test waits until the status line reports the render. It then reads the
  canvas with `getImageData`. The reference is `render_map_with_files` of `crates/bindings-wasm`,
  compiled natively. Its input is the sample mapfile, its five GeoJSON sources, and the files the
  page hands in, at the page's viewport. The test compares width, height, and every byte. The
  canvas keeps its pixels premultiplied, so the bytes compare equal only where every pixel is
  opaque, as the sample's are. This test cites G-14.
- **The diagnostics.** A second case types a mapfile with a known fault into the page's mapfile
  field and presses *Render*. The test checks that `#diagnostics` lists, in order, one entry per
  diagnostic the native render returns, each with its severity, code, and message. This case cites
  only this ADR, not G-14, because it does not use the sample.
- **Where it runs.** The variable `FORAGER_BROWSER_TEST=required` makes the test fail when it finds
  no ChromeDriver (`CHROMEWEBDRIVER`, else `PATH`), no Chrome, or no bundle. Without the variable,
  the test returns at once with a note on stderr. So `cargo test --workspace` and
  `scripts/check-all.sh` pass in the Dev Container. `scripts/check-browser.sh` runs this test
  alone, and builds the bundle with `scripts/build-web.sh` first where the variable is set. The
  CI job sets the variable and runs that script. `scripts/check-all.sh` runs it too, because the
  local gate runs every script a workflow runs (the rule `docs/check-lists` of
  `scripts/check-docs.sh`).

**Out of scope:**

- browsers other than Chrome;
- the tile view (Leaflet), whose seams the test `neighbouring_tiles_meet_without_a_seam` of
  `crates/bindings-wasm` holds natively;
- the layout and styling of the page;
- a browser in the Dev Container, which stays a matter of
  [OPS-dev-container-adds-rust](OPS-dev-container-adds-rust.md).

## Alternatives considered

- **Playwright** (one of the maintainer's two candidates). Mature, and it waits on the page by
  itself. It can drive the runner's Chrome through the channel `chrome`, without a download.
  `@playwright/test` 1.65.1 brings three npm packages, all Apache-2.0. But it needs Node in CI,
  which [OPS-dev-container-adds-rust](OPS-dev-container-adds-rust.md) installs nowhere, and an npm
  lock file that `cargo deny` does not check. A revision of this draft chose it. The maintainer
  weighed both on 2026-11-15 and kept the client of the project's own, so that the test stays in
  Rust under the checks the workspace has.
- **wasm-bindgen-test in a headless browser** (the maintainer's other candidate). It runs Rust
  tests of the bindings inside the browser, but it never opens `examples/web/index.html`, its
  JavaScript, or its canvas. G-14 would stay unverified.
- **fantoccini against ChromeDriver.** A maintained, complete WebDriver client (MIT or Apache-2.0).
  It passes `cargo deny` but adds 81 packages and an async runtime to the test. The test uses a
  handful of the protocol's calls.
- **headless_chrome (the DevTools protocol).** It needs no driver binary, but the DevTools protocol
  is Chrome's own and versioned with it, unlike the W3C standard. The crate also adds a WebSocket
  client and its dependencies.
- **Headless Chrome from a bash check** (`--dump-dom` or `--screenshot` with a time budget). No
  crates. But the DOM does not hold the canvas pixels, a screenshot depends on fonts, layout, and
  the colour scheme, and a fixed time budget is flaky.
- **A browser and ChromeDriver in the Dev Container.** The test would run in `check-all.sh` too. But
  it supersedes part of [OPS-dev-container-adds-rust](OPS-dev-container-adds-rust.md), enlarges the
  image, and adds a binary pinned outside Cargo
  ([OPS-pinned-stable-rust-toolchain](OPS-pinned-stable-rust-toolchain.md)).
- **A canvas checked only for its size and for not being empty.** Simpler and robust, but it would
  pass a page that draws the wrong image. The byte comparison costs nothing more, since the native
  render equals the wasm32 one (G-9).
- **The sample changed to give a diagnostic.** It would serve both checks with one input, but it
  spoils the example and breaks the test that holds it clean.

## Sources / Prior art

- W3C, *WebDriver*, Recommendation: <https://www.w3.org/TR/webdriver2/>. Its endpoints: New
  Session, Navigate To, Execute Script, Delete Session; the capability `goog:chromeOptions` with
  `--headless=new`.
- GitHub, runner image *Ubuntu 24.04*, version 20261111.370.1:
  <https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md>. It lists
  Google Chrome and ChromeDriver 157.0.8103.50, and
  `CHROMEWEBDRIVER=/usr/local/share/chromedriver-linux64`.
- fantoccini 0.22.1 (<https://crates.io/crates/fantoccini>) and headless_chrome 1.0.22
  (<https://crates.io/crates/headless_chrome>), compared on their licences and features with
  `cargo info`. fantoccini was trial-added without TLS, with tokio, and checked with
  `cargo deny check` and its count of added packages.
- The page `examples/web/index.html` and the dev server `crates/dev-server/src/main.rs`, read for
  their ids, their status line, and their port option.

## Consequences

- Positive: G-14 is verified, and the traceability check lists no pending criterion. The page, the
  glue wasm-bindgen generates, and the bundle are tested together in a real browser, which
  neither the conformance test in wasmi nor the native tests reach. No crate is added, and Q-5
  holds.
- Negative / trade-offs: the test runs only in CI. Locally it passes without running, and a
  session that changes the page learns of a break only from CI. The WebDriver client is code the
  project keeps, small but its own. Chrome follows the runner image, not a pin. A change of the
  image can break the test, which then fails visibly. Canvas pixels read back by `getImageData`
  could differ from those put in where a browser applies colour management to the canvas. Chrome
  does not do so for `putImageData` and `getImageData` in sRGB. Should it ever, the comparison
  fails, and this ADR needs revising.
- Follow-ups: whether to bring a browser into the Dev Container, if CI-only proves too slow a loop;
  whether the tile view gets a browser check of its own.

## Enforcement

The test of `crates/dev-server/tests/browser.rs` that compares the canvas with the native render of
the sample cites G-14 and this ADR with `Verifies:`. The case of the diagnostics list cites this
ADR. The CI job of the browser test in `.github/workflows/checks.yml` runs
`scripts/check-browser.sh` with `FORAGER_BROWSER_TEST=required`, so a missing ChromeDriver,
browser, or bundle fails it.
`crates/arch-test/tests/dependencies.rs` cites this ADR and refuses any dev-dependency of
`crates/dev-server` other than `forager-bindings-wasm` and `serde_json`. That keeps the client
without a crate of its own.
