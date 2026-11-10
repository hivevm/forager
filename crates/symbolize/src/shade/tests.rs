use std::rc::Rc;

use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point, PremultipliedImage};
use forager_style_ir::{Bound, Extent, Rgba, Style, Symbol, SymbolKind, SymbolRef};

use super::*;
use crate::canvas::Canvas;
use crate::extent::View;
use crate::symbol::Pixmaps;
use crate::tile::Tiles;

const BLUE: Rgba = Rgba::opaque(30, 60, 200);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A canvas without symbols, of 100 × 100.
fn plain() -> Canvas<'static> {
    Canvas::new(&[], 100, 100, 1.0)
}

fn line() -> Vec<Vec<Point>> {
    vec![vec![p(0.0, 0.0), p(10.0, 0.0)]]
}

fn colored(color: Option<Rgba>, outline: Option<Rgba>) -> Style {
    Style {
        color: color.map(Bound::Value),
        outline_color: outline.map(Bound::Value),
        ..Style::default()
    }
}

#[test]
fn a_filled_polygon_is_outlined_at_its_fill_s_alpha() {
    let fill = Rgba {
        a: 100,
        ..Rgba::opaque(200, 80, 80)
    };
    let mut ops = Vec::new();
    let style = Style {
        width: Some(Bound::Value(2.0)),
        ..colored(Some(fill), Some(BLACK))
    };
    let view = View::for_image(
        forager_style_ir::Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        },
        11,
        11,
    )
    .unwrap();
    assert!(shade(&mut ops, line(), &style, &view, &mut plain()));
    // The polygon shape is one fill item, through MapServer's default gamma, the 0.75 of
    // agg2CreateImage's msGetOutputFormatOption(format, "GAMMA", "0.75").
    assert!(matches!(
        ops[0],
        DrawOp::Fill {
            color,
            gamma: Some(0.75),
            ..
        } if color == fill
    ));
    match &ops[1] {
        DrawOp::Stroke {
            color, cap, join, ..
        } => {
            assert_eq!(*color, Rgba { a: 100, ..BLACK });
            assert_eq!((*cap, *join), (LineCap::Round, LineJoin::Round));
        }
        DrawOp::Fill { .. }
        | DrawOp::Hatch(_)
        | DrawOp::Glyphs { .. }
        | DrawOp::Pixmap { .. }
        | DrawOp::ResampledPixmap { .. }
        | DrawOp::TiledFill { .. } => {
            panic!("the outline is a stroke")
        }
    }
}

/// A view of 0..100 on 101 × 101 pixels: a map unit is a pixel, y downwards from 100.
fn unit_view() -> View {
    View::for_image(
        Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        },
        101,
        101,
    )
    .unwrap()
}

/// A filled circle, symbol 1.
fn circle() -> Symbol {
    Symbol {
        name: None,
        kind: SymbolKind::Ellipse,
        paths: vec![vec![[1.0, 1.0]]],
        extent: Some([1.0, 1.0]),
        filled: true,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }
}

fn hatch_symbol() -> Symbol {
    Symbol {
        name: None,
        kind: forager_style_ir::SymbolKind::Hatch,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }
}

#[test]
fn a_hatch_is_bounded_on_the_image_s_own_size_not_one_rebuilt_from_the_extent() {
    // An image of 1001 × 1001 pixels whose view spans 0..99 at a pixel a unit: the extent
    // over the cellsize would give 100 × 100. A hatch a tenth of a pixel apart, a pixel
    // wide, over a polygon wider than the image lays some 10 060 lines of 1 006 pixels over
    // the image itself, past the bound on their length (Q-9), which the renderer measures
    // on 1001 × 1001; over 100 × 100 they would be some 110 000 pixels in all.
    let view = View {
        extent: Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 99.0,
            max_y: 99.0,
        },
        cellsize: 1.0,
    };
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(0.1)),
        ..colored(Some(BLUE), None)
    };
    let symbols = [hatch_symbol()];
    let ring = |side: f64| {
        vec![vec![
            p(-10.0, -10.0),
            p(side, -10.0),
            p(side, side),
            p(-10.0, side),
        ]]
    };
    let shaded = |width: u32, ring| {
        let mut canvas = Canvas::new(&symbols, width, width, 1.0);
        let mut ops = Vec::new();
        shade(&mut ops, ring, &style, &view, &mut canvas)
    };
    assert!(!shaded(1001, ring(1100.0)), "refused on 1001 × 1001");
    // For contrast, not deciding: the hatch over a polygon about an image that is truly
    // 100 × 100 is drawn. The view is built by hand, since `View::for_image` always gives
    // an extent that rebuilds the image's own size, and the criterion is the case where the
    // two disagree.
    assert!(shaded(100, ring(110.0)), "drawn on 100 × 100");
}

fn stroke(op: &DrawOp) -> (f64, &Dashes, &[Vec<Point>]) {
    match op {
        DrawOp::Stroke {
            width,
            dashes,
            paths,
            ..
        } => (*width, dashes, paths),
        other => panic!("{other:?}"),
    }
}

#[test]
fn at_a_factor_a_polygon_moves_by_its_offset_scaled_and_its_outline_bounds_stay() {
    // msDrawShadeSymbol, maprendering.c:771-799, at a scale factor of 2: the OFFSET
    // times the scale factor; the outline's WIDTH times it, the factor itself for a WIDTH of
    // 0, bounded by MAXWIDTH and MINWIDTH as written.
    let outlined = |width: f64, max_width: Option<f64>| {
        let style = Style {
            width: Some(Bound::Value(width)),
            max_width,
            offset: Some([Bound::Value(3.0), Bound::Value(4.0)]),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let mut ops = Vec::new();
        let mut canvas = Canvas::new(&[], 101, 101, 2.0);
        let ring = vec![vec![p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0)]];
        assert!(shade(&mut ops, ring, &style, &unit_view(), &mut canvas));
        ops
    };
    let ops = outlined(3.0, None);
    let DrawOp::Fill { rings, .. } = &ops[0] else {
        panic!("{ops:?}");
    };
    assert_eq!(rings[0], vec![p(16.0, 18.0), p(26.0, 18.0), p(26.0, 28.0)]);
    assert_eq!(stroke(&ops[1]).0, 6.0);
    assert_eq!(
        stroke(&outlined(0.0, None)[1]).0,
        2.0,
        "WIDTH 0 is the factor"
    );
    assert_eq!(
        stroke(&outlined(3.0, Some(5.0))[1]).0,
        5.0,
        "MAXWIDTH 5 stays 5"
    );
}

/// A style with `MINWIDTH` and `MAXWIDTH` whose `WIDTH` is bound to a feature's property that
/// reads as no number, `nan`, which atof() reads as NaN.
fn bounded_nan(min_width: f64, max_width: f64, outline: Option<Rgba>) -> Style {
    Style {
        width: Some(Bound::Value(forager_style_ir::c::atof(b"nan"))),
        min_width: Some(min_width),
        max_width: Some(max_width),
        ..colored(Some(BLUE), outline)
    }
}

#[test]
fn a_polygon_of_a_symbol_past_the_last_or_of_a_name_no_symbol_carries_draws_nothing() {
    // msDrawShadeSymbol draws nothing for an index past the symbols; a style document's name
    // no symbol carries draws nothing either. Symbol 0 and a name found fill the polygon.
    let symbols = [Symbol {
        name: Some("Dot".to_owned()),
        ..circle()
    }];
    let by = |symbol| Style {
        symbol: Some(Bound::Value(symbol)),
        color: Some(Bound::Value(BLUE)),
        outline_color: Some(Bound::Value(BLACK)),
        ..Style::default()
    };
    for (symbol, drawn) in [
        (SymbolRef::Index(2), false),
        (SymbolRef::Name("nowhere".to_owned()), false),
        (SymbolRef::Index(0), true),
        (SymbolRef::Name("DOT".to_owned()), true),
    ] {
        let mut ops = Vec::new();
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        let ring = vec![vec![p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0)]];
        assert!(shade(
            &mut ops,
            ring,
            &by(symbol.clone()),
            &unit_view(),
            &mut canvas
        ));
        assert_eq!(!ops.is_empty(), drawn, "{symbol:?}: {ops:?}");
    }
}

#[test]
fn a_polygon_bounds_its_outline_as_ms_draw_shade_symbol_does() {
    // msDrawShadeSymbol, maprendering.c:796-799: the WIDTH times the scale factor (NaN is not
    // 0), MS_MIN with MAXWIDTH, then MS_MAX with MINWIDTH, neither scaled. A NaN width becomes
    // MAXWIDTH as written, and a MINWIDTH above the MAXWIDTH wins.
    let outline = |style: &Style| {
        let mut ops = Vec::new();
        let mut canvas = Canvas::new(&[], 101, 101, 2.0);
        let ring = vec![vec![p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0)]];
        assert!(shade(&mut ops, ring, style, &unit_view(), &mut canvas));
        stroke(&ops[1]).0
    };
    assert_eq!(outline(&bounded_nan(2.0, 5.0, Some(BLACK))), 5.0);
    let crossed = Style {
        width: Some(Bound::Value(4.0)),
        ..bounded_nan(6.0, 3.0, Some(BLACK))
    };
    assert_eq!(outline(&crossed), 6.0, "MINWIDTH last");
}

/// A pixmap symbol of `path`.
fn pixmap_symbol(path: Option<&str>) -> Symbol {
    Symbol {
        name: Some("pin".into()),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: path.map(str::to_owned),
        transparent: None,
        span: None,
    }
}

#[test]
fn a_polygon_s_pixmap_is_moved_by_its_offset_and_draws_neither_colour_nor_outline() {
    // msDrawShadeSymbol: the offset moves the polygon (770-783), computeSymbolStyle leaves a
    // pixmap no colours (46-47), and the tile's branch draws no outline.
    let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
    let pixmaps: Pixmaps = [("pin.png".to_owned(), Ok(image))].into();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        color: Some(Bound::Value(BLUE)),
        outline_color: Some(Bound::Value(BLACK)),
        offset: Some([Bound::Value(3.0), Bound::Value(-2.0)]),
        ..Style::default()
    };
    let mut tiles = Tiles::default();
    let mut canvas = Canvas::new(&symbols, 101, 101, 2.0)
        .with_pixmaps(&pixmaps)
        .with_tiles(&mut tiles);
    let mut ops = Vec::new();
    let ring = vec![vec![p(10.0, 10.0), p(90.0, 10.0), p(90.0, 90.0)]];
    assert!(shade(&mut ops, ring, &style, &unit_view(), &mut canvas));
    match ops.as_slice() {
        [DrawOp::TiledFill { rings, tile }] => {
            assert_eq!(rings[0][0], p(16.0, 6.0), "the offset scaled by the factor");
            // Without a SIZE: the image's height 3 at the factor 2, over 3: 8 × 6.
            assert_eq!((tile.width(), tile.height()), (8, 6));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_polygon_s_pixmap_without_its_image_or_past_its_bound_draws_nothing_and_says_why() {
    // Verifies: Q-9
    let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
    let pixmaps: Pixmaps = [
        ("pin.png".to_owned(), Ok(image)),
        ("gone.png".to_owned(), Err("it does not load".to_owned())),
    ]
    .into();
    let symbols = [
        pixmap_symbol(Some("gone.png")),
        pixmap_symbol(Some("pin.png")),
    ];
    let style = |index, gap| Style {
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        color: Some(Bound::Value(BLUE)),
        gap: Some(gap),
        ..Style::default()
    };
    let drawn = |style: &Style, tiles: Option<&mut Tiles>| {
        let canvas = Canvas::new(&symbols, 101, 101, 1.0).with_pixmaps(&pixmaps);
        let mut canvas = match tiles {
            Some(tiles) => canvas.with_tiles(tiles),
            None => canvas,
        };
        let mut ops = Vec::new();
        let ring = vec![vec![p(10.0, 10.0), p(90.0, 10.0), p(90.0, 90.0)]];
        assert!(shade(&mut ops, ring, style, &unit_view(), &mut canvas));
        assert!(ops.is_empty(), "{ops:?}");
        let refused = canvas.pixmaps_refused();
        assert_eq!(refused.len(), 1);
        refused[0].2.to_owned()
    };
    let mut tiles = Tiles::default();
    assert_eq!(drawn(&style(1, 0.0), Some(&mut tiles)), "it does not load");
    let why = drawn(&style(2, 4097.0), Some(&mut tiles));
    assert!(why.contains("4097 × 4097"), "{why}");
    assert!(why.contains("past the limit of 4096 × 4096"), "{why}");
    assert!(drawn(&style(2, 0.0), None).contains("no tiles"));
}
