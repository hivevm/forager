//! A plain line and a filled polygon whose vertex the renderer's clip drops, end to end: a vertex
//! that is no finite number in pixels, or two whose difference overflows (ARCH-one-clip-to-image).
//! The layer that draws them carries the warning `path-not-drawn` once
//! (ARCH-third-party-failures-guarded), and a path the clip keeps whole carries none.

use std::collections::BTreeMap;
use std::future::Future;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Span, Style,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// A map of 100 × 100 pixels of `extent`, whose one layer `paths` draws its features in `mode`
/// in the blue `style`.
fn map(extent: Extent, mode: RenderMode, style: Style) -> Map {
    let layer = Layer {
        name: Some("paths".into()),
        render_mode: Some(mode),
        status: Some(LayerStatus::On),
        source: Some(SourceRef {
            kind: "local".into(),
            ..SourceRef::default()
        }),
        classes: vec![Class {
            styles: vec![Style {
                color: Some(Bound::Value(Rgba::opaque(30, 60, 200))),
                ..style
            }],
            ..Class::default()
        }],
        span: Some(Span::new("m.map", 2, 1)),
        ..Layer::default()
    };
    Map {
        extent: Some(extent),
        size: Some([100, 100]),
        image_color: Some(WHITE),
        layers: vec![layer],
        ..Map::default()
    }
}

/// A line layer of the extent 0..0.099, a thousandth of a map unit a pixel, that leaves its lines
/// unclipped, `PROCESSING "POLYLINE_NO_CLIP"`, so that the engine hands them on to the renderer as
/// they are, whatever their vertices give in pixels.
fn line_map() -> Map {
    let extent = Extent {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.099,
        max_y: 0.099,
    };
    let mut map = map(
        extent,
        RenderMode::Line,
        Style {
            width: Some(Bound::Value(3.0)),
            ..Style::default()
        },
    );
    map.layers[0].unclipped_lines = true;
    map
}

/// A polygon layer of the extent ±8e307, so wide that the class's clip buffer of 1003 pixels,
/// from the style's `SIZE` of 1000, grows it past every finite number: the engine's clip keeps
/// every vertex, and one past the extent comes out no finite number in pixels.
fn polygon_map() -> Map {
    let extent = Extent {
        min_x: -8e307,
        min_y: -8e307,
        max_x: 8e307,
        max_y: 8e307,
    };
    map(
        extent,
        RenderMode::Polygon,
        Style {
            size: Some(Bound::Value(1000.0)),
            ..Style::default()
        },
    )
}

/// `map` rendered with its layer's one feature of the GeoJSON geometry `geometry`.
fn rendered(map: &Map, geometry: &str) -> Rendered {
    let sources = BTreeMap::from([("paths".to_owned(), GeoJsonSource::parse(geometry).unwrap())]);
    ready(render(map, &sources, AggRenderer::new()))
}

/// How many pixels of the rendered image are not white.
fn drawn(rendered: &Rendered) -> usize {
    let image = rendered.image.as_ref().unwrap();
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|&&pixel| pixel != [255; 4])
        .count()
}

/// Asserts that `rendered` carries exactly one diagnostic, the warning `path-not-drawn` on the
/// layer `paths` for `paths`, such as "1 path is", and draws what the clip left.
fn warns_once(rendered: &Rendered, paths: &str) {
    let codes: Vec<&str> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["path-not-drawn"], "{:?}", rendered.diagnostics);
    let warning = &rendered.diagnostics[0];
    for part in ["`paths`", paths] {
        assert!(
            warning.message.contains(part),
            "{part}: {}",
            warning.message
        );
    }
    assert_eq!(warning.span, Some(Span::new("m.map", 2, 1)));
    assert!(drawn(rendered) > 0, "the points left are drawn");
}

/// A line from (10, 50) to (90, 50) in pixels through each of `between`, in map units.
fn line(between: &[f64]) -> String {
    let mut xs = vec![0.01];
    xs.extend(between);
    xs.push(0.09);
    let coordinates: Vec<String> = xs.iter().map(|x| format!("[{x:e}, 0.049]")).collect();
    format!(
        r#"{{"type": "LineString", "coordinates": [{}]}}"#,
        coordinates.join(", ")
    )
}

#[test]
fn a_line_the_clip_keeps_whole_warns_nothing() {
    // Verifies: ARCH-third-party-failures-guarded
    let whole = rendered(&line_map(), &line(&[0.05]));
    assert!(whole.diagnostics.is_empty(), "{:?}", whole.diagnostics);
    assert!(drawn(&whole) > 0);
}

#[test]
fn a_line_with_a_vertex_that_is_no_finite_number_in_pixels_warns_once() {
    // Verifies: ARCH-one-clip-to-image, ARCH-third-party-failures-guarded, Q-3
    // 1e306 map units are 1e309 pixels, past every finite number. Two such vertices, in one line
    // and in another part, still warn once.
    warns_once(&rendered(&line_map(), &line(&[1e306])), "1 path is drawn");
    let two_parts = r#"{"type": "MultiLineString", "coordinates": [
        [[0.01, 0.049], [1e306, 0.049], [0.09, 0.049]],
        [[0.01, 0.02], [0.05, 1e306], [0.09, 0.02]]
    ]}"#;
    warns_once(&rendered(&line_map(), two_parts), "2 paths are drawn");
}

#[test]
fn a_line_whose_two_vertices_differ_past_every_finite_number_warns_once() {
    // Verifies: ARCH-one-clip-to-image, ARCH-third-party-failures-guarded, Q-3
    // ±1e305 map units are ±1e308 pixels, each finite, their difference not.
    warns_once(
        &rendered(&line_map(), &line(&[1e305, -1e305])),
        "1 path is drawn",
    );
}

/// A square of 25 pixels a side around the image's centre, with a vertex at `far` map units
/// right of the centre between two of its corners.
fn polygon(far: f64) -> String {
    let c = 1.6e306 * 12.5;
    format!(
        r#"{{"type": "Polygon", "coordinates": [[[{m:e}, {m:e}], [{c:e}, {m:e}], [{far:e}, 0],
            [{c:e}, {c:e}], [{m:e}, {c:e}], [{m:e}, {m:e}]]]}}"#,
        m = -c,
    )
}

#[test]
fn a_polygon_the_clip_keeps_whole_warns_nothing() {
    // Verifies: ARCH-third-party-failures-guarded
    let whole = rendered(&polygon_map(), &polygon(1.6e306 * 20.0));
    assert!(whole.diagnostics.is_empty(), "{:?}", whole.diagnostics);
    assert!(drawn(&whole) > 0);
}

#[test]
fn a_filled_polygon_with_a_vertex_that_is_no_finite_number_in_pixels_warns_once() {
    // Verifies: ARCH-one-clip-to-image, ARCH-third-party-failures-guarded, Q-3
    // 1.7e308 map units lie 2.5e308 map units right of the extent's left edge, past every finite
    // number, before they are scaled to pixels.
    warns_once(
        &rendered(&polygon_map(), &polygon(1.7e308)),
        "1 path is drawn",
    );
}
