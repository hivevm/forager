//! The adversarial probe of [`JoinProbe`]'s join bound, rebuilt from the second review of
//! revision-2026-11-16 ticket 21: random roads on an image of 4000 pixels, each brushed by the
//! probe and by agg-rust unbounded, with overflow checks on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use agg_rust::renderer_base::RendererBase;

use super::super::*;

/// A xorshift generator, so that a run is the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn within(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }
}

/// A tile of `width` by `height` pixels of one opaque colour, laying an image.
fn tile(width: u32, height: u32) -> Rc<Tile> {
    let pixels = (0..width * height)
        .flat_map(|_| [200, 40, 40, 255])
        .collect();
    let image = Rc::new(PremultipliedImage::new(width, height, pixels).unwrap());
    let mark = DrawOp::Pixmap {
        image,
        left: 0,
        top: 0,
    };
    Rc::new(Tile::new(width, height, vec![mark]).unwrap())
}

fn pattern(tile: &Tile) -> LineImagePattern<BilinearRgba8> {
    LineImagePattern::<BilinearRgba8>::with_source(&Source(tile.image(built_tile)))
}

fn feed(rasterizer: &mut RasterizerOutlineAa, fed: &[(i32, i32)]) {
    rasterizer.move_to(fed[0].0, fed[0].1);
    for &(x, y) in &fed[1..] {
        rasterizer.line_to(x, y);
    }
}

/// Whether [`JoinProbe`] admits `fed`, the subpixels of a piece.
fn admitted(pattern: &LineImagePattern<BilinearRgba8>, fed: &[(i32, i32)]) -> bool {
    let mut rasterizer = RasterizerOutlineAa::new();
    feed(&mut rasterizer, fed);
    let mut probe = JoinProbe::new(pattern);
    rasterizer.render(&mut probe, false);
    probe.within
}

/// Whether agg-rust draws `fed` unbounded, as [`AggRenderer::brushed_line`] draws a piece the
/// probe admits, without a panic. The image is 8 by 8 pixels: the renderer clips each pixel it
/// blends, not the pixels its interpolators step over, so the arithmetic is that of any image.
fn draws(pattern: &LineImagePattern<BilinearRgba8>, fed: &[(i32, i32)]) -> bool {
    let mut pixels = vec![255u8; 8 * 8 * 4];
    catch_unwind(AssertUnwindSafe(|| {
        let mut base = RendererBase::new(PixfmtMapserver::new(&mut pixels, 8, 8));
        let mut carried = Carried {
            inner: RendererOutlineImage::new(&mut base, pattern),
            pattern,
        };
        carried.set_start(0);
        let mut rasterizer = RasterizerOutlineAa::new();
        feed(&mut rasterizer, fed);
        rasterizer.render(&mut carried, false);
    }))
    .is_ok()
}

/// A road of 2 to 5 segments of 2 to 3000 pixels each on an image of 4000 pixels, each turning
/// from the one before by up to `turn` degrees either way.
fn road(rng: &mut Rng, turn: f64) -> Vec<(f64, f64)> {
    let mut points = vec![(rng.within(0.0, 4000.0), rng.within(0.0, 4000.0))];
    let mut heading = rng.within(0.0, 360.0);
    let segments = 2 + rng.next() % 4;
    for i in 0..segments {
        let (x, y) = points[points.len() - 1];
        for _ in 0..50 {
            let towards = if i == 0 {
                heading
            } else {
                heading + rng.within(-turn, turn)
            };
            let length = rng.within(2.0, 3000.0);
            let (dx, dy) = (towards.to_radians().cos(), towards.to_radians().sin());
            let end = (x + length * dx, y + length * dy);
            if (0.0..=4000.0).contains(&end.0) && (0.0..=4000.0).contains(&end.1) {
                points.push(end);
                heading = towards;
                break;
            }
        }
    }
    points
}

fn subpixels(points: &[(f64, f64)], scale: f64) -> Vec<(i32, i32)> {
    let (x0, y0) = points[0];
    points
        .iter()
        .map(|&(x, y)| {
            (
                line_coord_sat(x0 + (x - x0) * scale),
                line_coord_sat(y0 + (y - y0) * scale),
            )
        })
        .collect()
}

/// What the probe found over the roads of one class of turns.
#[derive(Debug, Default)]
struct Found {
    roads: usize,
    /// Admitted by the probe and drawn by agg-rust with an overflow: undefined behaviour of AGG.
    holes: usize,
    /// Refused by the probe, though agg-rust draws them without an overflow.
    losses: usize,
    /// Refused by the probe, and agg-rust overflows on them.
    saved: usize,
    /// Roads scaled about their first point to the largest scale the probe admits, by a binary
    /// search, and drawn by agg-rust there: each must draw without an overflow.
    at_edge: usize,
    holes_at_edge: usize,
}

/// `roads` random roads for each class of turns, up to 30°, 90°, 120°, 150°, and 170°, the first
/// `edges` of each also scaled to the probe's edge.
fn probe(roads: usize, edges: usize, seed: u64) -> Vec<(u32, Found)> {
    let mut rng = Rng(seed);
    let tiles: Vec<Rc<Tile>> = [1, 3, 17, 64]
        .into_iter()
        .flat_map(|w| [1, 2, 6, 40, 130, 300].map(|h| tile(w, h)))
        .collect();
    let patterns: Vec<_> = tiles.iter().map(|t| pattern(t)).collect();
    [30, 90, 120, 150, 170]
        .into_iter()
        .map(|turn| {
            let mut found = Found::default();
            for road_of_class in 0..roads {
                let pattern = &patterns[(rng.next() % patterns.len() as u64) as usize];
                let points = road(&mut rng, f64::from(turn));
                let fed = subpixels(&points, 1.0);
                if filtered(&fed).is_empty() {
                    continue;
                }
                found.roads += 1;
                match (admitted(pattern, &fed), draws(pattern, &fed)) {
                    (true, false) => found.holes += 1,
                    (false, true) => found.losses += 1,
                    (false, false) => found.saved += 1,
                    (true, true) => {}
                }
                // The edge: the largest scale admitted, if a scale up to 2^12 is refused.
                if road_of_class >= edges {
                    continue;
                }
                let (mut low, mut high) = (1.0 / 64.0, 4096.0);
                if !admitted(pattern, &subpixels(&points, low))
                    || admitted(pattern, &subpixels(&points, high))
                {
                    continue;
                }
                for _ in 0..40 {
                    let middle = (low * high).sqrt();
                    if admitted(pattern, &subpixels(&points, middle)) {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                found.at_edge += 1;
                if !draws(pattern, &subpixels(&points, low)) {
                    found.holes_at_edge += 1;
                }
            }
            (turn, found)
        })
        .collect()
}

/// A few roads a class on each run of the tests; the full probe, which ticket 04 of
/// backlog-deviations ran against the box bound, takes 1500 a class, each also at its edge,
/// optimized with overflow checks on:
///
/// ```text
/// FORAGER_ROADS=1500 FORAGER_ROADS_AT_EDGE=1500 cargo test \
///   --config 'profile.probe.inherits="release"' --config 'profile.probe.overflow-checks=true' \
///   --profile probe -p forager-render-agg --lib random_roads -- --nocapture
/// ```
///
/// It prints, for each class, the roads refused that agg-rust draws without an overflow (the
/// losses) and those it overflows on (saved). A panic agg-rust raises on a road it overflows on
/// is caught, and its message kept with the test's output.
#[test]
fn random_roads_the_probe_admits_draw_without_an_overflow() {
    // Verifies: ADR-0049, Q-3
    let count = |name: &str, otherwise: usize| {
        std::env::var(name)
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(otherwise)
    };
    let (roads, edges) = (
        count("FORAGER_ROADS", 20),
        count("FORAGER_ROADS_AT_EDGE", 5),
    );
    let found = probe(roads, edges, 0x5eed_2026_1119);
    for (turn, found) in &found {
        eprintln!("turns up to {turn}°: {found:?}");
    }
    for (turn, found) in &found {
        assert_eq!((found.holes, found.holes_at_edge), (0, 0), "{turn}°");
    }
}
