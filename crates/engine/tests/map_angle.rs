// The tests of the angles turned by a rotated map work their values out from MapServer's
// pointLayerDrawShape, lineLayerDrawShape, and polygonLayerDrawShape, src/mapdraw.c, of the
// release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! On a map with an `ANGLE`, the angles of a point layer's styles and labels, and of a line or a
//! polygon layer's labels, turned by the map's: an angle that is not 0 less the map's, and 0 as
//! it is. Each is compared with the same symbol or label drawn at the turned angle, at the same
//! pixels, on a twin that is not rotated: `misc/rot_out.map`'s view, 400 × 300 rotated by 30°,
//! against the extent 0..399 × 0..299, one map unit a pixel.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, InlineFeature, Label, Layer, LayerProjection, LayerStatus, Map,
    RenderMode, Rgba, ShapeType, Style, Symbol, SymbolKind, SymbolRef,
};

/// This crate's directory: the run time's `CARGO_MANIFEST_DIR`, else the compiled-in path.
fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const BLUE: Rgba = Rgba::opaque(30, 60, 200);

/// `misc/rot_out.map`'s projection, map and layer alike, so that only the geotransform moves a
/// point.
const UTM: &str = "+proj=utm +zone=11 +datum=WGS84";

/// The map's angle.
const MAP_ANGLE: f64 = 30.0;

/// The flipped geotransform of `misc/rot_out.map` at 30°, as the C's formulas give it: from the
/// frame of the image, y up, into the map's coordinates.
const GT: [f64; 6] = [
    479_078.344_914_828_37,
    7.907_188_469_336_169,
    -4.565_217_391_304_347,
    4_762_145.878_251_339,
    4.565_217_391_304_341,
    7.907_188_469_336_179,
];

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The test font of the label crate, `fonts/sans.ttf`, a subset of DejaVu Sans.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str() != "fonts/sans.ttf" {
            return Err(SourceError::new("no such file"));
        }
        let file = manifest_dir().join("../label/tests/fixtures/dejavu-sans-subset.ttf");
        std::fs::read(file).map_err(|e| SourceError::new(e.to_string()))
    }
}

/// The pixel `(x, y)` of the image, in the twin's coordinates and in the rotated map's, which
/// the geotransform takes it to.
fn at(x: f64, y: f64) -> ([f64; 2], [f64; 2]) {
    let fy = 300.0 - y;
    let rotated = [
        GT[0] + x * GT[1] + fy * GT[2],
        GT[3] + x * GT[4] + fy * GT[5],
    ];
    ([x, 299.0 - y], rotated)
}

/// A map of one layer in `mode` of `parts` at the pixels they name, drawn in `class`: rotated by
/// [`MAP_ANGLE`] where `rotated` is set, else the twin.
fn map(rotated: bool, mode: RenderMode, features: &[Vec<Vec<(f64, f64)>>], class: Class) -> Map {
    let shape_type = match mode {
        RenderMode::Point => ShapeType::Point,
        RenderMode::Line => ShapeType::Line,
        _ => ShapeType::Polygon,
    };
    let pick = |&(x, y): &(f64, f64)| {
        let (twin, turned) = at(x, y);
        if rotated { turned } else { twin }
    };
    let features = features
        .iter()
        .map(|parts| InlineFeature {
            shape_type,
            parts: parts.iter().map(|p| p.iter().map(pick).collect()).collect(),
            properties: Vec::new(),
            text: None,
            span: None,
        })
        .collect();
    let (extent, projection, angle) = if rotated {
        let extent = Extent {
            min_x: 478_300.0,
            min_y: 4_762_880.0,
            max_x: 481_650.0,
            max_y: 4_765_610.0,
        };
        (extent, Some(UTM.to_owned()), Some(MAP_ANGLE))
    } else {
        let extent = Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 399.0,
            max_y: 299.0,
        };
        (extent, None, None)
    };
    Map {
        extent: Some(extent),
        size: Some([400, 300]),
        image_color: Some(WHITE),
        projection: projection.clone(),
        angle,
        fonts: [("sans".to_owned(), "fonts/sans.ttf".to_owned())]
            .into_iter()
            .collect(),
        symbols: vec![Symbol {
            name: Some("v-line".into()),
            kind: SymbolKind::Vector,
            paths: vec![vec![[0.0, 0.0], [5.0, 10.0], [10.0, 0.0]]],
            extent: Some([10.0, 10.0]),
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }],
        layers: vec![Layer {
            name: Some("data".into()),
            render_mode: Some(mode),
            status: Some(LayerStatus::On),
            projection: projection.map(LayerProjection::Proj4),
            classes: vec![class],
            features,
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn render(map: &Map) -> Rendered {
    let sources = BTreeMap::<String, GeoJsonSource>::new();
    let rendered = ready(render_with_files(map, &sources, &Files, AggRenderer::new()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    rendered
}

/// A class of the v-line symbol at `angle`, 20 pixels high, stroked 2 wide.
fn marker(angle: Option<f64>) -> Class {
    Class {
        styles: vec![Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: Some(Bound::Value(20.0)),
            width: Some(Bound::Value(2.0)),
            color: Some(Bound::Value(BLUE)),
            angle: angle.map(Bound::Value),
            ..Style::default()
        }],
        ..Class::default()
    }
}

/// The image of a point layer's marker at `angle` at the pixel (200, 150).
fn marker_image(rotated: bool, angle: Option<f64>) -> Vec<u8> {
    let point = vec![vec![vec![(200.0, 150.0)]]];
    let rendered = render(&map(rotated, RenderMode::Point, &point, marker(angle)));
    rendered.image.unwrap().pixels
}

#[test]
fn a_point_styles_angle_is_turned_by_the_maps() {
    // pointLayerDrawShape (src/mapdraw.c:2050-2060): an angle of 50 on a map of 30 draws at 20.
    let turned = marker_image(true, Some(50.0));
    assert_eq!(turned, marker_image(false, Some(50.0 - MAP_ANGLE)));
    assert_ne!(turned, marker_image(false, Some(50.0)));
}

#[test]
fn a_point_style_of_angle_0_or_of_the_maps_stays_upright() {
    // An angle of 0 is not turned; the map's own angle is turned to 0. Both draw as the symbol
    // written, while the map turns the shapes.
    let upright = marker_image(false, None);
    assert_eq!(marker_image(true, None), upright);
    assert_eq!(marker_image(true, Some(0.0)), upright);
    assert_eq!(marker_image(true, Some(MAP_ANGLE)), upright);
    assert_ne!(
        marker_image(true, Some(MAP_ANGLE)),
        marker_image(false, Some(MAP_ANGLE))
    );
}

/// A class of a label `Forager` at `angle`.
fn labelled(angle: f64) -> Class {
    Class {
        labels: vec![Label {
            text: Some("Forager".into()),
            font: Some(Bound::Value("sans".to_owned())),
            size: Some(Bound::Value(12.0)),
            color: Some(Bound::Value(BLUE)),
            angle: Some(Bound::Value(angle)),
            ..Label::default()
        }],
        ..Class::default()
    }
}

/// The boxes the label cache drew the labels of `features` in, a layer in `mode` labelled at
/// `angle`.
fn label_boxes(
    rotated: bool,
    mode: RenderMode,
    features: &[Vec<Vec<(f64, f64)>>],
    angle: f64,
) -> Vec<[f64; 4]> {
    let rendered = render(&map(rotated, mode, features, labelled(angle)));
    rendered
        .labels
        .iter()
        .map(|label| label.drawn.expect("the label is drawn"))
        .collect()
}

/// Whether the boxes `a` and `b` are the same to a millionth of a pixel, which the geotransform's
/// round trip of the features moves them by.
fn same(a: &[[f64; 4]], b: &[[f64; 4]]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6))
}

#[test]
fn a_point_labels_angle_is_turned_by_the_maps() {
    // pointLayerDrawShape (:2050-2054) subtracts the map's angle from a label's that is not 0 and
    // adds it back after the shape, and the label cache tests the label at that angle: each of
    // the two features' labels is turned once. The labels of line and polygon layers, which
    // lineLayerDrawShape and polygonLayerDrawShape turn, are not drawn yet.
    let points = vec![vec![vec![(120.0, 100.0)]], vec![vec![(280.0, 220.0)]]];
    let turned = label_boxes(true, RenderMode::Point, &points, 50.0);
    assert_eq!(turned.len(), 2);
    let expected = label_boxes(false, RenderMode::Point, &points, 50.0 - MAP_ANGLE);
    assert!(same(&turned, &expected), "{turned:?} {expected:?}");
    let unturned = label_boxes(false, RenderMode::Point, &points, 50.0);
    assert!(!same(&turned, &unturned));
}

#[test]
fn a_label_of_angle_0_is_not_turned() {
    let point = vec![vec![vec![(200.0, 150.0)]]];
    let turned = label_boxes(true, RenderMode::Point, &point, 0.0);
    assert!(same(
        &turned,
        &label_boxes(false, RenderMode::Point, &point, 0.0)
    ));
}
