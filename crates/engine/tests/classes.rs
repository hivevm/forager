//! The class that draws each feature, the layer's filter, and the properties bound to each
//! feature, through the engine's display list (ADR-0031).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render};
use forager_render::{DisplayItem, DrawOp, Renderer, RgbaImage};
use forager_style_ir::{
    Bound, Class, Expr, Extent, Layer, LayerStatus, Map, Op, RenderMode, Rgba, Style,
};

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);

/// A renderer that keeps what it is asked to draw.
#[derive(Clone, Default)]
struct Recorder(Rc<RefCell<Vec<DisplayItem>>>);

impl Renderer for Recorder {
    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}

    fn draw(&mut self, item: &DisplayItem) {
        self.0.borrow_mut().push(item.clone());
    }

    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        }
    }
}

fn layer(mode: RenderMode, filter: Option<Expr>, classes: Vec<Class>) -> Layer {
    Layer {
        name: Some("data".into()),
        render_mode: Some(mode),
        status: Some(LayerStatus::On),
        filter,
        classes,
        ..Layer::default()
    }
}

fn class(expression: Option<Expr>, styles: Vec<Style>) -> Class {
    Class {
        expression,
        styles,
        ..Class::default()
    }
}

fn filled(color: Rgba, size: f64) -> Style {
    Style {
        color: Some(Bound::Value(color)),
        size: Some(Bound::Value(size)),
        ..Style::default()
    }
}

/// A point at x = 10, 20, … for each property object, in order.
fn points(properties: &[&str]) -> String {
    let features: Vec<String> = properties
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                r#"{{"type": "Feature", "properties": {p},
                    "geometry": {{"type": "Point", "coordinates": [{}, 50]}}}}"#,
                10 * (i + 1)
            )
        })
        .collect();
    format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    )
}

fn draw(layer: Layer, geojson: &str) -> (Vec<DisplayItem>, Rendered) {
    let map = Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        }),
        size: Some([101, 101]),
        layers: vec![layer],
        ..Map::default()
    };
    let sources: BTreeMap<String, GeoJsonSource> =
        [("data".to_owned(), GeoJsonSource::parse(geojson).unwrap())].into();
    let recorder = Recorder::default();
    let rendered = forager_ports::poll_once(render(&map, &sources, recorder.clone())).unwrap();
    let items = recorder.0.borrow().clone();
    (items, rendered)
}

/// Each feature's fills: its place in the source from 1, the colour, and the width of the square.
fn fills(items: &[DisplayItem]) -> Vec<(u64, Rgba, f64)> {
    items
        .iter()
        .filter_map(|item| match &item.op {
            DrawOp::Fill { rings, color } => {
                let xs = rings[0].iter().map(|p| p.x);
                let width = xs.clone().fold(f64::MIN, f64::max) - xs.fold(f64::MAX, f64::min);
                Some((item.feature_id.unwrap() + 1, *color, width))
            }
            DrawOp::Stroke { .. } | DrawOp::Glyphs { .. } => None,
        })
        .collect()
}

fn number(name: &str) -> Expr {
    Expr::call(Op::ToNumber, vec![Expr::property(name)])
}

fn text(name: &str) -> Expr {
    Expr::call(Op::ToString, vec![Expr::property(name)])
}

#[test]
fn each_feature_is_drawn_with_the_first_class_that_matches() {
    // Verifies: ADR-0031
    let classes = vec![
        class(
            Some(Expr::call(Op::Gt, vec![number("n"), Expr::Number(1.0)])),
            vec![filled(RED, 2.0)],
        ),
        class(
            Some(Expr::call(
                Op::RegexpLike,
                vec![text("name"), "^r".into(), "i".into()],
            )),
            vec![filled(GREEN, 4.0)],
        ),
        class(
            Some(Expr::call(
                Op::In,
                vec![text("name"), Expr::List(vec!["lake".into(), "sea".into()])],
            )),
            vec![filled(BLUE, 6.0)],
        ),
    ];
    let geojson = points(&[
        r#"{"n": 2, "name": "Road"}"#,
        r#"{"n": "1", "name": "Road"}"#,
        r#"{"N": 0, "NAME": "sea"}"#,
        r#"{"n": 0, "name": "field"}"#,
    ]);
    let (items, rendered) = draw(layer(RenderMode::Point, None, classes), &geojson);
    // The first matches the first class and the second as well, the third the third class by
    // its names in another case, and the fourth no class: it is not drawn.
    assert_eq!(
        fills(&items),
        vec![(1, RED, 2.0), (2, GREEN, 4.0), (3, BLUE, 6.0)]
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
}

#[test]
fn a_class_without_an_expression_draws_every_feature_that_reaches_it() {
    let classes = vec![
        class(Some(Expr::Bool(false)), vec![filled(RED, 2.0)]),
        class(None, vec![filled(GREEN, 2.0)]),
    ];
    let (items, _) = draw(
        layer(RenderMode::Point, None, classes),
        &points(&["{}", "{}"]),
    );
    assert_eq!(fills(&items), vec![(1, GREEN, 2.0), (2, GREEN, 2.0)]);
}

#[test]
fn features_that_fail_the_layer_s_filter_are_not_drawn() {
    // Verifies: ADR-0031
    let filter = Expr::call(
        Op::Eq,
        vec![
            Expr::call(Op::CaseI, vec![text("kind")]),
            Expr::call(Op::CaseI, vec!["road".into()]),
        ],
    );
    let classes = vec![class(None, vec![filled(RED, 2.0)])];
    let geojson = points(&[r#"{"kind": "ROAD"}"#, r#"{"kind": "river"}"#, "{}"]);
    let (items, _) = draw(layer(RenderMode::Point, Some(filter), classes), &geojson);
    assert_eq!(fills(&items), vec![(1, RED, 2.0)]);
}

#[test]
fn bound_and_computed_sizes_and_colours_are_each_feature_s_own() {
    // Verifies: ADR-0031
    let bound = Style {
        color: Some(Bound::Binding {
            binding: "color".into(),
        }),
        size: Some(Bound::Binding {
            binding: "size".into(),
        }),
        ..Style::default()
    };
    // SIZE ([size] * 2), as the frontend writes msEvalDoubleExpression; COLOR by an expression.
    let computed = Style {
        color: Some(Bound::Expr(Expr::call(
            Op::Concat,
            vec!["0 ".into(), text("green"), " 0".into()],
        ))),
        size: Some(Bound::Expr(Expr::call(
            Op::ToNumber,
            vec![Expr::call(
                Op::Tostring,
                vec![
                    Expr::call(Op::Mul, vec![number("size"), Expr::Number(2.0)]),
                    "%g".into(),
                ],
            )],
        ))),
        ..Style::default()
    };
    let classes = vec![class(None, vec![bound, computed])];
    let geojson = points(&[
        r##"{"size": 4, "color": "#ff0000", "green": "255"}"##,
        r##"{"size": "", "color": "0 0 255", "green": "128"}"##,
    ]);
    let (items, _) = draw(layer(RenderMode::Point, None, classes), &geojson);
    assert_eq!(
        fills(&items),
        vec![
            (1, RED, 4.0),
            (1, GREEN, 8.0),
            // An empty size binds 1, as bindStyle sets it before it reads the value; the
            // expression reads it as 0.
            (2, BLUE, 1.0),
            (2, Rgba::opaque(0, 128, 0), 0.0),
        ]
    );
}

#[test]
fn a_line_layer_draws_cached_styles_after_every_feature_whatever_its_class() {
    // msDrawVectorLayer: a feature of a class of several styles draws its first at once and the
    // rest after every feature; one of a single style draws at once.
    let line = |color| Style {
        color: Some(Bound::Value(color)),
        ..Style::default()
    };
    let classes = vec![
        class(
            Some(Expr::call(Op::Eq, vec![text("k"), "a".into()])),
            vec![line(RED), line(GREEN)],
        ),
        class(None, vec![line(BLUE)]),
    ];
    let geojson = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {"k": "a"},
         "geometry": {"type": "LineString", "coordinates": [[0, 10], [100, 10]]}},
        {"type": "Feature", "properties": {"k": "b"},
         "geometry": {"type": "LineString", "coordinates": [[0, 20], [100, 20]]}}]}"#;
    let (items, _) = draw(layer(RenderMode::Line, None, classes), geojson);
    let strokes: Vec<(u64, Rgba)> = items
        .iter()
        .filter_map(|item| match &item.op {
            DrawOp::Stroke { color, .. } => Some((item.feature_id.unwrap() + 1, *color)),
            DrawOp::Fill { .. } | DrawOp::Glyphs { .. } => None,
        })
        .collect();
    assert_eq!(strokes, vec![(1, RED), (2, BLUE), (1, GREEN)]);
}

#[test]
fn a_pattern_the_crate_cannot_match_is_reported_once_per_render() {
    // Verifies: ADR-0031
    let classes = vec![
        class(
            Some(Expr::call(
                Op::RegexpLike,
                vec![text("name"), "(a)\\1".into(), "".into()],
            )),
            vec![filled(RED, 2.0)],
        ),
        class(None, vec![filled(GREEN, 2.0)]),
    ];
    let geojson = points(&[r#"{"name": "aa"}"#, r#"{"name": "b"}"#]);
    let (items, rendered) = draw(layer(RenderMode::Point, None, classes), &geojson);
    // The class never matches, so each feature falls to the next.
    assert_eq!(fills(&items), vec![(1, GREEN, 2.0), (2, GREEN, 2.0)]);
    let codes: Vec<_> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["pattern-unsupported"]);
    assert!(rendered.diagnostics[0].message.contains("class 1"));
    assert!(rendered.diagnostics[0].message.contains("a back-reference"));
}

#[test]
fn a_property_no_feature_has_reads_as_empty_and_the_layer_draws() {
    // MapServer reads "" for a field of the source's schema a feature has no value for; without a
    // schema, Forager reads every name a feature lacks so, whatever the features in view hold.
    let classes = vec![
        class(
            Some(Expr::call(Op::Eq, vec![text("nowhere"), "x".into()])),
            vec![filled(RED, 2.0)],
        ),
        class(None, vec![filled(GREEN, 2.0)]),
    ];
    let (items, rendered) = draw(
        layer(RenderMode::Point, None, classes),
        &points(&[r#"{"name": "a"}"#]),
    );
    assert_eq!(fills(&items), vec![(1, GREEN, 2.0)]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
}

/// The scale of [`draw`]'s map: 100 metres over 100 pixel steps at 72 dpi.
const SCALE: f64 = 100.0 / (100.0 / (72.0 * 39.3701));

#[test]
fn a_class_outside_its_scale_range_draws_no_feature() {
    // msShapeGetNextClass skips a class msScaleInBounds puts out of the map's scale.
    let ranged = |min: Option<f64>, max: Option<f64>, color| Class {
        min_scale_denom: min,
        max_scale_denom: max,
        ..class(None, vec![filled(color, 2.0)])
    };
    let classes = vec![
        ranged(None, Some(SCALE - 1.0), RED),
        ranged(Some(SCALE + 1.0), None, BLUE),
        ranged(Some(SCALE - 1.0), Some(SCALE + 1.0), GREEN),
    ];
    let (items, _) = draw(layer(RenderMode::Point, None, classes), &points(&["{}"]));
    assert_eq!(fills(&items), vec![(1, GREEN, 2.0)]);
}

#[test]
fn a_class_range_holds_its_minimum_and_not_its_maximum() {
    // msScaleInBounds: excluded at MAXSCALEDENOM, included at MINSCALEDENOM.
    let ranged = |min: Option<f64>, max: Option<f64>, color| Class {
        min_scale_denom: min,
        max_scale_denom: max,
        ..class(None, vec![filled(color, 2.0)])
    };
    let classes = vec![
        ranged(None, Some(SCALE), RED),
        ranged(Some(SCALE), None, GREEN),
    ];
    let (items, _) = draw(layer(RenderMode::Point, None, classes), &points(&["{}"]));
    assert_eq!(fills(&items), vec![(1, GREEN, 2.0)]);
}

#[test]
fn a_layer_outside_its_scale_range_draws_nothing() {
    // msLayerIsVisible: a layer past its MAXSCALEDENOM, or at its MINSCALEDENOM, is skipped.
    let one = || vec![class(None, vec![filled(RED, 2.0)])];
    for (min, max) in [(None, Some(SCALE - 1.0)), (Some(SCALE), None)] {
        let layer = Layer {
            min_scale_denom: min,
            max_scale_denom: max,
            ..layer(RenderMode::Point, None, one())
        };
        let (items, _) = draw(layer, &points(&["{}"]));
        assert!(items.is_empty(), "{min:?} {max:?}");
    }
    // At its MAXSCALEDENOM a layer draws.
    let at_max = Layer {
        max_scale_denom: Some(SCALE),
        ..layer(RenderMode::Point, None, one())
    };
    assert_eq!(
        fills(&draw(at_max, &points(&["{}"])).0),
        vec![(1, RED, 2.0)]
    );
}

#[test]
fn a_layer_whose_only_class_is_out_of_scale_is_skipped() {
    // msLayerIsVisible skips a layer when no class passes its own check: the scale on the only
    // class's MINSCALEDENOM fails it, where msScaleInBounds would admit it.
    let classes = vec![Class {
        min_scale_denom: Some(SCALE),
        ..class(None, vec![filled(RED, 2.0)])
    }];
    let (items, _) = draw(layer(RenderMode::Point, None, classes), &points(&["{}"]));
    assert!(items.is_empty());
}

#[test]
fn a_style_outside_its_scale_range_draws_nothing() {
    let out = Style {
        max_scale_denom: Some(SCALE - 1.0),
        ..filled(RED, 2.0)
    };
    let classes = vec![class(None, vec![out, filled(GREEN, 2.0)])];
    let (items, _) = draw(layer(RenderMode::Point, None, classes), &points(&["{}"]));
    assert_eq!(fills(&items), vec![(1, GREEN, 2.0)]);
}

#[test]
fn a_line_class_whose_outline_width_is_bound_is_cached_whatever_its_value() {
    // msDrawVectorLayer caches a class whose one style binds OUTLINEWIDTH, even where a feature
    // binds 0: the second feature's line of the first layer then draws after the third's.
    let bound = Style {
        color: Some(Bound::Value(RED)),
        outline_width: Some(Bound::Binding {
            binding: "w".into(),
        }),
        ..Style::default()
    };
    let classes = vec![
        class(
            Some(Expr::call(Op::Eq, vec![text("k"), "a".into()])),
            vec![bound],
        ),
        class(
            None,
            vec![Style {
                color: Some(Bound::Value(BLUE)),
                ..Style::default()
            }],
        ),
    ];
    let geojson = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {"k": "a", "w": "0"},
         "geometry": {"type": "LineString", "coordinates": [[0, 10], [100, 10]]}},
        {"type": "Feature", "properties": {"k": "b"},
         "geometry": {"type": "LineString", "coordinates": [[0, 20], [100, 20]]}}]}"#;
    let (items, _) = draw(layer(RenderMode::Line, None, classes), geojson);
    let strokes: Vec<(u64, Rgba)> = items
        .iter()
        .filter_map(|item| match &item.op {
            DrawOp::Stroke { color, .. } => Some((item.feature_id.unwrap() + 1, *color)),
            DrawOp::Fill { .. } | DrawOp::Glyphs { .. } => None,
        })
        .collect();
    // The cached feature draws its first pass at once, then its line after every feature.
    assert_eq!(strokes, vec![(1, RED), (2, BLUE), (1, RED)]);
}

#[test]
fn a_pattern_of_a_style_s_expression_is_reported_once_per_render() {
    // Verifies: ADR-0031
    let size = Expr::call(
        Op::ToNumber,
        vec![Expr::call(
            Op::Concat,
            vec![
                Expr::call(
                    Op::ToString,
                    vec![Expr::call(
                        Op::RegexpLike,
                        vec![text("name"), "a++".into(), "".into()],
                    )],
                ),
                "".into(),
            ],
        )],
    );
    let style = Style {
        size: Some(Bound::Expr(size)),
        ..filled(RED, 2.0)
    };
    let classes = vec![class(None, vec![style])];
    let geojson = points(&[r#"{"name": "a"}"#, r#"{"name": "b"}"#]);
    let (items, rendered) = draw(layer(RenderMode::Point, None, classes), &geojson);
    // The size that fails is 0, as msEvalDoubleExpression gives it.
    assert_eq!(fills(&items), vec![(1, RED, 0.0), (2, RED, 0.0)]);
    let codes: Vec<_> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["pattern-unsupported"]);
    assert!(
        rendered.diagnostics[0]
            .message
            .contains("style 1 of its class 1")
    );
}

#[test]
fn a_text_past_the_limit_is_reported_and_its_class_never_matches() {
    // Verifies: Q-9
    let long = "x".repeat(forager_expr::MAX_TEXT_BYTES / 4 + 1);
    let four = Expr::call(Op::Concat, vec![text("a"), text("a"), text("a"), text("a")]);
    let classes = vec![
        class(
            Some(Expr::call(Op::Eq, vec![four, "".into()])),
            vec![filled(RED, 2.0)],
        ),
        class(None, vec![filled(GREEN, 2.0)]),
    ];
    let geojson = points(&[
        &format!(r#"{{"a": "{long}"}}"#),
        &format!(r#"{{"a": "{long}"}}"#),
    ]);
    let (items, rendered) = draw(layer(RenderMode::Point, None, classes), &geojson);
    assert_eq!(fills(&items), vec![(1, GREEN, 2.0), (2, GREEN, 2.0)]);
    let codes: Vec<_> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["text-too-long"]);
}
