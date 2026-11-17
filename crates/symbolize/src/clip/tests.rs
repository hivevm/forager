use std::rc::Rc;

use forager_render::PremultipliedImage;
use forager_style_ir::{Bound, RenderMode, SymbolKind, SymbolRef};

use super::*;
use crate::draw::clip_pixmaps;

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn style(size: Option<f64>, width: Option<f64>) -> Style {
    Style {
        size: size.map(Bound::Value),
        width: width.map(Bound::Value),
        ..Style::default()
    }
}

#[test]
fn the_clip_buffer_of_a_line_counts_no_offset_and_no_polygon_s_2() {
    // msDrawShape grows a style's size by its offset only for a polygon shape, and adds 2 for a
    // polygon alone: a style of SIZE 10 and WIDTH 3 offset by (4, 5) gives a line
    // max(1, max(10, 3)) + 1 = 11.
    let offset = Style {
        offset: Some([Bound::Value(4.0), Bound::Value(5.0)]),
        ..style(Some(10.0), Some(3.0))
    };
    let clip = Clip::for_class(
        &[style(None, None), offset],
        &[],
        None,
        Factors::new(1.0, 1.0),
    );
    assert_eq!(clip.line_buffer, 11.0);
    assert_eq!(clip.buffer, 18.0);
}

#[test]
fn a_line_is_cut_where_it_leaves_the_rectangle_and_begun_again_where_it_comes_back() {
    // msClipPolylineRect: a line that leaves ends at the edge, one that comes back begins there,
    // its parts lines of their own.
    let (min, max) = (p(0.0, 0.0), p(10.0, 10.0));
    let line = vec![
        p(-10.0, 5.0),
        p(5.0, 5.0),
        p(15.0, 5.0),
        p(15.0, 8.0),
        p(5.0, 8.0),
    ];
    assert_eq!(
        clip_polyline_rect(&[line], min, max),
        vec![
            vec![p(0.0, 5.0), p(5.0, 5.0), p(10.0, 5.0)],
            vec![p(10.0, 8.0), p(5.0, 8.0)]
        ]
    );
    // Wholly inside, by the bounds of all the lines, they are kept as they are.
    let inside = vec![vec![p(1.0, 1.0), p(9.0, 9.0)], vec![p(0.0, 10.0)]];
    assert_eq!(clip_polyline_rect(&inside, min, max), inside);
    // Wholly to one side, a segment adds nothing.
    let outside = vec![p(-5.0, 1.0), p(-1.0, 9.0), p(5.0, 20.0)];
    assert!(clip_polyline_rect(&[outside], min, max).is_empty());
}

#[test]
fn a_segment_is_cut_in_x_then_in_y_by_what_the_cut_in_x_leaves() {
    // clipLine on (-10, -10) to (20, 5): in x, slope 0.5, the start to (0, -5), then the end, by
    // the slope of the cut segment, to (10, 0); in y, the start, below, by the slope x over y of
    // 2, to (10, 0): the segment becomes the corner point twice.
    let (min, max) = (p(0.0, 0.0), p(10.0, 10.0));
    assert_eq!(
        clip_line(p(-10.0, -10.0), p(20.0, 5.0), min, max),
        Some((p(10.0, 0.0), p(10.0, 0.0)))
    );
    assert_eq!(
        clip_polyline_rect(&[vec![p(-10.0, -10.0), p(20.0, 5.0)]], min, max),
        vec![vec![p(10.0, 0.0), p(10.0, 0.0)]]
    );
    // A diagonal across a corner: (-5, 0) to (5, 10) from (0, 5).
    assert_eq!(
        clip_line(p(-5.0, 0.0), p(5.0, 10.0), min, max),
        Some((p(0.0, 5.0), p(5.0, 10.0)))
    );
}

#[test]
fn a_line_beyond_the_extent_is_clipped_in_map_units_by_the_line_clip_buffer() {
    // A view of 0..100 at a cellsize of 1: the line from x = -1000 is cut at -2, the extent less
    // the buffer of 2, before it is turned into pixels; a layer of POLYLINE_NO_CLIP keeps it.
    let view = View::for_image(
        forager_style_ir::Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        },
        101,
        101,
    )
    .unwrap();
    let c = |x: f64, y: f64| Coord { x, y };
    let line = vec![c(-1000.0, 50.0), c(50.0, 50.0)];
    let clip = Clip {
        line_buffer: 2.0,
        ..Clip::default()
    };
    let image = (101.0, 101.0);
    let clipped = line_paths(std::slice::from_ref(&line), &view, image, &clip, true);
    assert_eq!(clipped, vec![vec![p(-2.0, 50.0), p(50.0, 50.0)]]);
    let kept = line_paths(std::slice::from_ref(&line), &view, image, &clip, false);
    assert_eq!(kept, vec![vec![p(-1000.0, 50.0), p(50.0, 50.0)]]);
    // With a GEOMTRANSFORM in the class, the image grown by the buffer cuts it, in pixels.
    let in_pixels = Clip {
        in_pixels: true,
        ..clip
    };
    let right = vec![c(50.0, 50.0), c(1000.0, 50.0)];
    let clipped = line_paths(&[right], &view, image, &in_pixels, true);
    assert_eq!(clipped, vec![vec![p(50.0, 50.0), p(103.0, 50.0)]]);
}

#[test]
fn the_clip_buffer_is_msdrawshape_s() {
    // A plain style: max(1, max(-1, 1)) + 1 = 2. A hatch of SIZE 10 and WIDTH 3 offset by
    // (4, 5): max(1, max(10, 3)) + 5 + 1 = 16. The class: 16, and 2 more for a polygon.
    let plain = style(None, None);
    let hatched = Style {
        offset: Some([Bound::Value(4.0), Bound::Value(5.0)]),
        ..style(Some(10.0), Some(3.0))
    };
    let clip = Clip::for_class(&[plain, hatched], &[], None, Factors::new(1.0, 1.0));
    assert_eq!(clip.buffer, 18.0);
    assert!(!clip.in_pixels);
    // MINSIZE above the rest counts as it is: 12.5 + 1 rounds half to even, to 14, and 16 with
    // the 2.
    let small = Style {
        min_size: Some(12.5),
        ..style(None, None)
    };
    assert_eq!(
        Clip::for_class(&[small], &[], None, Factors::new(1.0, 1.0)).buffer,
        16.0
    );
}

#[test]
fn a_polygon_is_clipped_as_msclippolygonrect_clips_it() {
    // A square from -10 to 10 cut to the rectangle from 0 to 20: msClipPolygonRect's walk adds,
    // edge by edge, the corner the bottom edge enters by, (0, 0), where the right edge enters,
    // (10, 0), and the ends inside, and closes the ring.
    let square = vec![
        p(-10.0, -10.0),
        p(10.0, -10.0),
        p(10.0, 10.0),
        p(-10.0, 10.0),
        p(-10.0, -10.0),
    ];
    let clipped = clip_polygon_rect(&[square], p(0.0, 0.0), p(20.0, 20.0));
    assert_eq!(
        clipped,
        vec![vec![
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(0.0, 0.0),
        ]]
    );
    // Rings wholly inside are kept as they are, and a ring wholly outside is dropped.
    let inside = vec![p(1.0, 1.0), p(5.0, 1.0), p(5.0, 5.0)];
    assert_eq!(
        clip_polygon_rect(std::slice::from_ref(&inside), p(0.0, 0.0), p(20.0, 20.0)),
        vec![inside.clone()]
    );
    let outside = vec![p(30.0, 30.0), p(40.0, 30.0), p(40.0, 40.0)];
    let kept = clip_polygon_rect(&[inside, outside], p(0.0, 0.0), p(20.0, 20.0));
    assert_eq!(kept.len(), 1);
}

#[test]
fn at_a_factor_the_clip_buffer_is_scaled() {
    // msDrawShape, mapdraw.c:2489-2499, at a scale and a resolution factor of 2: the largest
    // size, grown by the offset, times the scale factor; MINSIZE and MINWIDTH times the
    // resolution factor. A plain style: 1 × 2 + 1 = 3; the hatch: (10 + 5) × 2 + 1 = 31; the
    // class 31 + 2.
    let plain = style(None, None);
    let hatched = Style {
        offset: Some([Bound::Value(4.0), Bound::Value(5.0)]),
        ..style(Some(10.0), Some(3.0))
    };
    let clip = Clip::for_class(&[plain.clone(), hatched], &[], None, Factors::new(2.0, 2.0));
    assert_eq!(clip.buffer, 33.0);
    assert_eq!(
        Clip::for_class(&[plain], &[], None, Factors::new(2.0, 2.0)).buffer,
        5.0
    );
    // MINSIZE 6.25 is 12.5: + 1 rounds half to even, to 14, and 16 with the 2.
    let small = Style {
        min_size: Some(6.25),
        ..style(None, None)
    };
    assert_eq!(
        Clip::for_class(&[small], &[], None, Factors::new(2.0, 2.0)).buffer,
        16.0
    );
}

#[test]
fn minwidth_counts_toward_the_clip_buffer_as_minsize_does() {
    // msDrawShape, mapdraw.c:2489-2499: the larger of MINSIZE and MINWIDTH, scaled by the
    // resolution factor, where it is larger than the size. MINWIDTH 10 at a factor of 2 is 20,
    // + 1 is 21, and 23 with the 2; a MINSIZE below it does not count.
    let wide = Style {
        min_width: Some(10.0),
        min_size: Some(3.0),
        ..style(None, None)
    };
    assert_eq!(
        Clip::for_class(&[wide], &[], None, Factors::new(2.0, 2.0)).buffer,
        23.0
    );
}

#[test]
fn a_pixmap_counts_in_the_clip_buffer_at_its_image_s_height() {
    // msDrawShape's maxsize takes msSymbolGetDefaultSize, for a pixmap the image's height, 48 of
    // an image 16 wide: MS_NINT(48 + 1), plus 2 for a polygon. Without its image, 1 counts, and
    // the hatch's SIZE 10 is the largest: MS_NINT(10 + 1) + 2.
    let tall = Rc::new(PremultipliedImage::new(16, 48, vec![0; 16 * 48 * 4]).unwrap());
    let pixmaps: Pixmaps = [("tall.png".to_owned(), Ok(tall))].into();
    let refused: Pixmaps = [("tall.png".to_owned(), Err("gone".to_owned()))].into();
    let symbol = |kind, image: Option<&str>| Symbol {
        name: None,
        kind,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: image.map(str::to_owned),
        transparent: None,
        span: None,
    };
    let symbols = [
        symbol(SymbolKind::Hatch, None),
        symbol(SymbolKind::Pixmap, Some("tall.png")),
    ];
    let style = |index, size| Style {
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        size,
        ..Style::default()
    };
    let class = [style(1, Some(Bound::Value(10.0))), style(2, None)];
    let buffer =
        |pixmaps| Clip::for_class(&class, &symbols, pixmaps, Factors::new(1.0, 1.0)).buffer;
    assert_eq!(buffer(Some(&pixmaps)), 51.0);
    assert_eq!(buffer(Some(&refused)), 13.0);
    assert_eq!(buffer(None), 13.0);
    // At a factor it is scaled as every size is: MS_NINT(48 × 2 + 1) + 2.
    let scaled = Clip::for_class(&class, &symbols, Some(&pixmaps), Factors::new(2.0, 2.0)).buffer;
    assert_eq!(scaled, 99.0);
    // The engine loads those a hatched class counts, and none of another polygon class; a line
    // layer's class counts them all, its lines clipped by the buffer.
    let polygon = RenderMode::Polygon;
    assert_eq!(clip_pixmaps(polygon, &class, &symbols), ["tall.png"]);
    assert!(clip_pixmaps(polygon, &class[1..], &symbols).is_empty());
    assert_eq!(
        clip_pixmaps(RenderMode::Line, &class[1..], &symbols),
        ["tall.png"]
    );
    assert!(clip_pixmaps(RenderMode::Point, &class, &symbols).is_empty());
    let line = Clip::for_class(
        &class[1..],
        &symbols,
        Some(&pixmaps),
        Factors::new(1.0, 1.0),
    );
    assert_eq!(line.line_buffer, 49.0);
}

#[test]
fn a_hatched_polygon_s_rings_reach_pixels_as_ms_transform_shape_simplify_puts_them() {
    // msTransformShapeSimplify, src/mapprimitive.cpp:1002-1040, on a view of cellsize 0.1:
    // each vertex by inv_cs = 10 (953), x = 0.3 at 3 where 0.3 / 0.1 is 2.9999999999999996;
    // (0.32, 9.7), a fifth of a pixel from the second vertex kept, is dropped; the last two are
    // kept; a ring of fewer than four vertices keeps none.
    let view = View::for_image(
        forager_style_ir::Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        },
        101,
        101,
    )
    .unwrap();
    assert_eq!(view.cellsize, 0.1);
    let c = |x: f64, y: f64| Coord { x, y };
    let ring = vec![
        c(5.0, 9.7),
        c(0.3, 9.7),
        c(0.32, 9.7),
        c(0.3, 5.0),
        c(5.0, 5.0),
        c(5.0, 9.7),
    ];
    let three = vec![c(1.0, 1.0), c(2.0, 1.0), c(1.0, 1.0)];
    let clip = Clip {
        buffer: 2.0,
        line_buffer: 0.0,
        in_pixels: false,
    };
    let rings = hatch_rings(&[ring, three], &view, (101.0, 101.0), &clip);
    assert_eq!(rings.len(), 1);
    let xs: Vec<f64> = rings[0].iter().map(|p| p.x).collect();
    assert_eq!(xs, vec![50.0, 3.0, 3.0, 50.0, 50.0]);
}
