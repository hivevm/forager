//! What the label cache costs (Q-9): the entries its tests visit, counted as the boxes it asks
//! for, and the memory it allocates, under a counting allocator, for boxes far larger than the
//! image and boxes turned inside out by a negative buffer. One test, so that no other test's
//! allocations are counted with it.

use std::cell::Cell;

use forager_label::{
    Anchor, Member, Members, Offsets, PointLabel, Position, TextBox, draw_label_cache,
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
                partials: true,
                test_point: false,
                marker: None,
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
    // Verifies: Q-9
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
}
