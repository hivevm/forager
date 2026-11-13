# ADR-0033's Enforcement names the noise measurement where it no longer lies

Observed: the Enforcement of ADR-0033 names the noise measurement "the ignored test of
`crates/cli/tests/msautotest.rs`". Since ca2a20c it lies in `crates/cli/tests/conformance/noise/`,
which carries its `Verifies: ADR-0033`; `msautotest.rs` is still the test target that holds it. PR
#176 named the stale path, and the design revision of 2026-11-11 filed it as ticket 10. The ADR is
accepted and immutable, and the path is no new decision, so a supersession would restate a whole
ADR for one path.

The maintainer chose on 2026-11-12 to correct it at the next normalization rather than supersede
ADR-0033 now: the normalization restates the record, and its restated ADR-0033 names
`crates/cli/tests/conformance/noise/`. Until then a reader of the Enforcement finds the test by its
marker.

Where: docs/adr/0033-noise-measurement.md (Enforcement), crates/cli/tests/conformance/noise/,
crates/cli/tests/msautotest.rs.
