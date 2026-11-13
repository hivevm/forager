# A label's FORCE is held twice, on the member and on its text

Observed: the label cache's `Member` carries `force`, read only for a member without text
(`crates/label/src/cache.rs`), and its `PointLabel` carries `force` too, which `place` and
`Cache::place` read. The engine sets both from one value at each of its two sites
(`crates/engine/src/labels.rs`). `cache.rs` documents the pair.

The design revision of 2026-11-11 filed this as ticket 05, to hold FORCE once. Its Mode stops where
a test of placement would have to change, and every form of the change does: removing either field
breaks the build of `crates/label/tests/cache_pairwise.rs`, which builds and reads both, and its
generator draws a chance for each field, so dropping one draw shifts every case generated after it,
unless the draw is kept unused; the fixtures of `cache.rs`, `place.rs`, and `cache_bounds.rs` set
both. The maintainer chose on 2026-11-12 the smallest variant: the duplicate stays, as documented.

A later change that touches these tests anyway may move `force` onto `Member` alone, `place`
taking it as a parameter, and decide whether `cache_pairwise.rs` keeps the unused draw so that its
cases stay as they are.

Where: crates/label/src/cache.rs (`Member`, `Cache::place`), crates/label/src/place.rs (`place`),
crates/engine/src/labels.rs, crates/label/tests/cache_pairwise.rs.
