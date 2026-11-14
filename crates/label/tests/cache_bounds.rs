//! What the label cache costs (Q-9): the entries its tests visit, counted as the boxes it asks
//! for, and the memory it allocates, under a counting allocator, for boxes far larger than the
//! image, boxes turned inside out by a negative buffer, and labels drawn beyond the image. One
//! test, so that no other test's allocations are counted with it.

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
                }),
                force,
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

/// The labels drawn, in the order drawn, the boxes asked for in all, and the bytes allocated per
/// label at the peak.
fn run(cache: &Counted) -> (Vec<usize>, usize, usize) {
    let count = cache.members.len();
    let mut drawn = Vec::with_capacity(count);
    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    draw_label_cache(cache, SIZE, 0, |i, _| drawn.push(i));
    let bytes = ALLOCATOR.peak_usage() - before;
    (drawn, cache.asked.get(), bytes / count)
}

#[test]
fn large_inside_out_and_off_image_boxes_cost_the_cache_little_per_label() {
    // Verifies: Q-9, ADR-0032, ADR-draft-label-cache-edge-cells
    // Forced boxes wider and taller than the image, each held once, not in every cell.
    let (drawn, _, bytes) = run(&labels(10_000.0, 3000, true, true));
    assert_eq!(drawn.len(), LABELS);
    assert!(bytes < 64, "{bytes} bytes per forced large label");
    // Boxes inside out by a buffer of -1000, all at one point: they never meet one another, and
    // no test visits them.
    let (drawn, asked, bytes) = run(&labels(30.0, -1000, false, false));
    let asked = asked / LABELS;
    assert_eq!(drawn.len(), LABELS, "inside-out boxes meet nothing");
    assert!(asked < 10, "{asked} boxes asked for per label");
    assert!(bytes < 64, "{bytes} bytes per inside-out label");
    // Spread over the image, a buffer of -3000 alike.
    let (drawn, asked, bytes) = run(&labels(30.0, -3000, true, false));
    let asked = asked / LABELS;
    assert_eq!(drawn.len(), LABELS);
    assert!(asked < 10, "{asked} boxes asked for per label");
    assert!(bytes < 64, "{bytes} bytes per inside-out label");
    // Large boxes not forced: the first drawn covers the image and is the first every later test
    // meets.
    let (drawn, asked, _) = run(&labels(10_000.0, 0, true, false));
    let asked = asked / LABELS;
    assert_eq!(drawn.len(), 1);
    assert!(asked < 10, "{asked} boxes asked for per label");
    // MAX_LABELS labels offset 3000 pixels from their points, within the image: each drawn
    // label is filed by its box, not by its box grown to its point, which would put it in the
    // coarsest level, where every later test visits it (ADR-0032).
    let offset = spread_labels(MAX_LABELS, UPPER_LEFT, 3000);
    let mut drawn = 0;
    let left_out = draw_label_cache(&offset, SIZE, 0, |_, _| drawn += 1);
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
    let over = spread_labels(MAX_LABELS + 1, UPPER_LEFT, 3000);
    let mut visited = Vec::new();
    let left_out = walk_label_cache(&over, SIZE, 0, |i, _| visited.push(i));
    assert_eq!(left_out, 1);
    assert_eq!(visited.len(), MAX_LABELS);
    assert!(
        !visited.contains(&MAX_LABELS),
        "the one past the limit is not visited"
    );
    off_image_labels_cost_what_the_same_labels_cost_within_it();
}

/// `MAX_LABELS` labels drawn beyond the image, by offsets right and down, left and up, and of a
/// million pixels: the grid wraps around the image, so they spread over its cells as the same
/// labels unmoved within the image do, and ask for no more boxes than those; where their points
/// spread over the upper left, fewer than 12 (ADR-draft-label-cache-edge-cells). Held at the
/// image's edge, the case of 3000 pixels over the whole image asked for 4962 boxes per label.
/// A million pixels is a whole number of cells of the finest level, where these labels are
/// held, so they lie in the cells as the unmoved ones do and ask for exactly as many boxes;
/// 3000 is not, and moves the count a little either way, as an offset within the image does.
fn off_image_labels_cost_what_the_same_labels_cost_within_it() {
    for step in [UPPER_LEFT, WHOLE] {
        let (unmoved, within, _) = run(&spread_labels(MAX_LABELS, step, 0));
        for offset in [3000, -3000, 1_000_000, -1_000_000] {
            let (drawn, asked, bytes) = run(&spread_labels(MAX_LABELS, step, offset));
            let case = format!("points {step} pixels apart, offset {offset}");
            assert_eq!(
                drawn, unmoved,
                "{case}: the labels meet as within the image"
            );
            assert!(
                asked <= within,
                "{case}: {asked} boxes asked for, {within} within the image"
            );
            if offset.abs() == 1_000_000 {
                assert_eq!(asked, within, "{case}: whole cells away");
            }
            if step != WHOLE {
                assert!(
                    asked < 12 * MAX_LABELS,
                    "{case}: {asked} boxes asked for, 12 a label or more"
                );
            }
            assert!(bytes < 64, "{case}: {bytes} bytes per label");
        }
    }
}

/// The spacing of points over the image's upper left 1078 pixels square, and over the whole
/// image, 317 to a row.
const UPPER_LEFT: f64 = 3.4;
const WHOLE: f64 = 4096.0 / 317.0;

/// `count` labels of a text 30 by 11 pixels, offset `offset` pixels right of and below points
/// `step` pixels apart, 317 to a row from the image's upper left corner; may be partial, not
/// forced. At [`UPPER_LEFT`] and an offset of 3000, every label lies in the image.
fn spread_labels(count: usize, step: f64, offset: i32) -> Counted {
    let members = (0..count)
        .map(|i| Member {
            priority: 1,
            point: Point {
                x: (i % 317) as f64 * step,
                y: (i / 317) as f64 * step,
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
                    label: [offset, offset],
                },
                rotation: 0.0,
                buffer: 0,
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
