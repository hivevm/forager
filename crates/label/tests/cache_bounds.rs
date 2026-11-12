//! What the label cache costs (Q-9): the entries its tests visit, counted as the boxes it asks
//! for, and the memory it allocates, under a counting allocator, for boxes far larger than the
//! image and boxes turned inside out by a negative buffer. One test, so that no other test's
//! allocations are counted with it.

use std::cell::Cell;

use forager_label::{
    Anchor, MAX_LABELS, Member, Members, Offsets, PointLabel, Position, TextBox, draw_label_cache,
    walk_label_cache,
};
use forager_render::Point;
use peak_alloc::PeakAlloc;

#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

/// Labels whose boxes the cache asks for, counted.
struct Counted {
    members: Vec<Member>,
    asked: Cell<usize>,
}

impl Members for Counted {
    fn count(&self) -> usize {
        self.members.len()
    }
    fn priority(&self, index: usize) -> u8 {
        self.members[index].priority
    }
    fn member(&self, index: usize) -> Member {
        self.asked.set(self.asked.get() + 1);
        self.members[index]
    }
    fn same_text(&self, _: usize, _: usize) -> bool {
        false
    }
}

const SIZE: (u32, u32) = (4096, 4096);
const LABELS: usize = 4000;

/// `LABELS` labels of a text `width` pixels wide and as high, or 11 high where it is narrower, with
/// `buffer`, at points spread over the image
/// or all at its centre, forced or not.
fn labels(width: f64, buffer: i32, spread: bool, force: bool) -> Counted {
    let members = (0..LABELS)
        .map(|i| {
            let (x, y) = if spread {
                ((i % 64) as f64 * 64.0 + 32.0, (i / 64) as f64 * 64.0 + 32.0)
            } else {
                (2048.0, 2048.0)
            };
            Member {
                priority: 1,
                point: Point { x, y },
                label: Some(PointLabel {
                    position: Position::At(Anchor::Cc),
                    text: TextBox {
                        min_x: 0.0,
                        min_y: if width > 100.0 { 2.0 - width } else { -9.0 },
                        max_x: width,
                        max_y: 2.0,
                        num_lines: 1,
                    },
                    offsets: Offsets::default(),
                    rotation: 0.0,
                    buffer,
                    force,
                }),
                force: false,
                partials: true,
                test_point: false,
                marker: None,
                label_marker: None,
                min_distance: 0.0,
            }
        })
        .collect();
    Counted {
        members,
        asked: Cell::new(0),
    }
}

/// The labels drawn, the boxes asked for per label, and the bytes allocated per label at the
/// peak.
fn run(cache: &Counted) -> (usize, usize, usize) {
    let mut drawn = Vec::with_capacity(LABELS);
    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    draw_label_cache(cache, SIZE, |i, _| drawn.push(i));
    let bytes = ALLOCATOR.peak_usage() - before;
    (drawn.len(), cache.asked.get() / LABELS, bytes / LABELS)
}

#[test]
fn large_and_inside_out_boxes_cost_the_cache_little_per_label() {
    // Verifies: Q-9, ADR-0032
    // Forced boxes wider and taller than the image, each held once, not in every cell.
    let (drawn, _, bytes) = run(&labels(10_000.0, 3000, true, true));
    assert_eq!(drawn, LABELS);
    assert!(bytes < 64, "{bytes} bytes per forced large label");
    // Boxes inside out by a buffer of -1000, all at one point: they never meet one another, and
    // no test visits them.
    let (drawn, asked, bytes) = run(&labels(30.0, -1000, false, false));
    assert_eq!(drawn, LABELS, "inside-out boxes meet nothing");
    assert!(asked < 10, "{asked} boxes asked for per label");
    assert!(bytes < 64, "{bytes} bytes per inside-out label");
    // Spread over the image, a buffer of -3000 alike.
    let (drawn, asked, bytes) = run(&labels(30.0, -3000, true, false));
    assert_eq!(drawn, LABELS);
    assert!(asked < 10, "{asked} boxes asked for per label");
    assert!(bytes < 64, "{bytes} bytes per inside-out label");
    // Large boxes not forced: the first drawn covers the image and is the first every later test
    // meets.
    let (drawn, asked, _) = run(&labels(10_000.0, 0, true, false));
    assert_eq!(drawn, 1);
    assert!(asked < 10, "{asked} boxes asked for per label");
    // MAX_LABELS labels offset 3000 pixels from their points, within the image: each drawn
    // label is filed by its box, not by its box grown to its point, which would put it in the
    // coarsest level, where every later test visits it (ADR-0032).
    let offset = offset_labels(MAX_LABELS);
    let mut drawn = 0;
    let left_out = draw_label_cache(&offset, SIZE, |_, _| drawn += 1);
    let asked = offset.asked.get() / MAX_LABELS;
    assert_eq!(left_out, 0);
    assert_eq!(drawn, 2844);
    // Dense, each label's box near hundreds of others, a test visits a few more of them than in
    // the sparse cases above; filed by its box grown to its point, as before this change, every
    // test visited every label drawn, 2844 of them here, 2840 boxes asked for per label. The
    // counter counts the members asked for, which a visit now asks once and asked twice before.
    assert!(asked < 12, "{asked} boxes asked for per offset label");
    // One label past MAX_LABELS: the cache takes the first MAX_LABELS and reports the one it
    // left out, untested, without the engine.
    let over = offset_labels(MAX_LABELS + 1);
    let mut visited = Vec::new();
    let left_out = walk_label_cache(&over, SIZE, |i, _| visited.push(i));
    assert_eq!(left_out, 1);
    assert_eq!(visited.len(), MAX_LABELS);
    assert!(
        !visited.contains(&MAX_LABELS),
        "the one past the limit is not visited"
    );
}

/// `count` labels of a text 30 by 11 pixels, offset 3000 pixels right of and below points spread
/// over the image's upper left 1078 pixels square, so that every label lies in the image; may be
/// partial, not forced.
fn offset_labels(count: usize) -> Counted {
    let members = (0..count)
        .map(|i| Member {
            priority: 1,
            point: Point {
                x: (i % 317) as f64 * 3.4,
                y: (i / 317) as f64 * 3.4,
            },
            label: Some(PointLabel {
                position: Position::At(Anchor::Cc),
                text: TextBox {
                    min_x: 0.0,
                    min_y: -9.0,
                    max_x: 30.0,
                    max_y: 2.0,
                    num_lines: 1,
                },
                offsets: Offsets {
                    marker: [0.0, 0.0],
                    label: [3000, 3000],
                },
                rotation: 0.0,
                buffer: 0,
                force: false,
            }),
            force: false,
            partials: true,
            test_point: false,
            marker: None,
            label_marker: None,
            min_distance: 0.0,
        })
        .collect();
    Counted {
        members,
        asked: Cell::new(0),
    }
}
