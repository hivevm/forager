//! A layer in another projection than the map's, drawn in the map's: its features reprojected
//! after its source delivers them, and its source asked for the map's extent in its own
//! coordinates (ADR-0037).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;

use forager_engine::{Rendered, render};
use forager_ports::{
    Coord, DeclaredProjection, Feature, FeatureQuery, FeatureSource, Features, Geometry,
    Properties, SourceError,
};
use forager_proj::{Projection, Reprojection};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, InlineFeature, Layer, LayerProjection, LayerStatus, Map, RenderMode,
    Rgba, ShapeType, Style,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const GREEN: Rgba = Rgba::opaque(200, 220, 180);

const LONGLAT: &str = "+proj=longlat +datum=WGS84 +no_defs";
/// EPSG:3978, as the msautotest suite's `wxs/data/epsg2` defines it.
const LCC: &str = "+proj=lcc +lat_1=49 +lat_2=77 +lat_0=49 +lon_0=-95 +x_0=0 +y_0=0 +ellps=GRS80 \
                   +datum=NAD83 +units=m +no_defs";

/// The map's extent in longitude and latitude: 10° on a side over 101 pixels, a tenth of a degree
/// a pixel, from 70°W 40°N.
const MAP_EXTENT: Extent = Extent {
    min_x: -70.0,
    min_y: 40.0,
    max_x: -60.0,
    max_y: 50.0,
};

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a source that never waits")
}

/// A source in memory that records the extents it is asked for and delivers all its features.
struct Recording {
    features: Vec<Feature>,
    asked: RefCell<Vec<Extent>>,
    declared: DeclaredProjection,
}

impl Recording {
    fn new(geometries: Vec<Geometry>) -> Self {
        let features = geometries
            .into_iter()
            .zip(0..)
            .map(|(geometry, id)| Feature {
                id,
                geometry,
                properties: Properties::default(),
            })
            .collect();
        Self {
            features,
            asked: RefCell::new(Vec::new()),
            declared: DeclaredProjection::None,
        }
    }

    fn declaring(mut self, declared: DeclaredProjection) -> Self {
        self.declared = declared;
        self
    }
}

impl FeatureSource for Recording {
    async fn features(&self, query: &FeatureQuery) -> Result<Features, SourceError> {
        self.asked.borrow_mut().push(query.extent);
        Ok(Features {
            features: self.features.clone(),
            ..Features::default()
        })
    }

    fn declared_projection(&self) -> DeclaredProjection {
        self.declared.clone()
    }
}

/// A map in `map_projection` over `extent`, of one polygon layer `data` in `layer_projection`,
/// filled green.
fn map(map_projection: &str, extent: Extent, layer_projection: Option<&str>) -> Map {
    Map {
        extent: Some(extent),
        size: Some([101, 101]),
        image_color: Some(WHITE),
        projection: Some(map_projection.to_owned()),
        layers: vec![Layer {
            name: Some("data".into()),
            render_mode: Some(RenderMode::Polygon),
            status: Some(LayerStatus::On),
            projection: layer_projection.map(|p| LayerProjection::Proj4(p.to_owned())),
            classes: vec![Class {
                styles: vec![Style {
                    color: Some(Bound::Value(GREEN)),
                    ..Style::default()
                }],
                ..Class::default()
            }],
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn render_from(map: &Map, source: Recording) -> (Rendered, Vec<Extent>) {
    let sources = BTreeMap::from([("data".to_owned(), source)]);
    let rendered = ready(render(map, &sources, AggRenderer::new()));
    let asked = sources["data"].asked.take();
    (rendered, asked)
}

fn way(from: &str, to: &str) -> Reprojection {
    Reprojection::new(
        Projection::parse(from).unwrap(),
        Projection::parse(to).unwrap(),
    )
}

/// The square from 66°W 44°N to 62°W 46°N, its sides each of 20 points, in `projection`.
fn square_in(projection: &str) -> Vec<Coord> {
    let way = way(LONGLAT, projection);
    let mut ring = Vec::new();
    let corners = [(-66.0, 44.0), (-62.0, 44.0), (-62.0, 46.0), (-66.0, 46.0)];
    for (k, &(x0, y0)) in corners.iter().enumerate() {
        let (x1, y1) = corners[(k + 1) % 4];
        for i in 0..20 {
            let t = f64::from(i) / 20.0;
            let at = Coord {
                x: x0 + t * (x1 - x0),
                y: y0 + t * (y1 - y0),
            };
            ring.push(way.point(at).unwrap());
        }
    }
    ring.push(ring[0]);
    ring
}

/// The pixel of `lon` and `lat` in a map of [`MAP_EXTENT`].
fn pixel(lon: f64, lat: f64) -> (u32, u32) {
    let column = ((lon - MAP_EXTENT.min_x) * 10.0).round();
    let row = ((MAP_EXTENT.max_y - lat) * 10.0).round();
    (column as u32, row as u32)
}

fn assert_square_drawn(rendered: &Rendered) {
    let image = rendered.image.as_ref().unwrap();
    let at = |lon, lat| {
        let (x, y) = pixel(lon, lat);
        image.pixel(x, y)
    };
    assert_eq!(at(-64.0, 45.0), Some(GREEN), "inside the square");
    assert_eq!(at(-65.8, 44.2), Some(GREEN), "near its south-west corner");
    assert_eq!(at(-62.2, 45.8), Some(GREEN), "near its north-east corner");
    assert_eq!(at(-67.0, 45.0), Some(WHITE), "west of it");
    assert_eq!(at(-64.0, 47.0), Some(WHITE), "north of it");
    assert_eq!(at(-61.0, 43.0), Some(WHITE), "south-east of it");
}

#[test]
fn a_layer_in_another_projection_is_drawn_in_the_maps() {
    // Verifies: ADR-0037
    let source = Recording::new(vec![Geometry::Polygon(vec![square_in(LCC)])]);
    let (rendered, _) = render_from(&map(LONGLAT, MAP_EXTENT, Some(LCC)), source);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_square_drawn(&rendered);
}

#[test]
fn a_layer_without_a_projection_or_in_the_maps_is_not_reprojected() {
    // Verifies: ADR-0037
    let in_longlat = || Recording::new(vec![Geometry::Polygon(vec![square_in(LONGLAT)])]);
    for layer_projection in [None, Some(LONGLAT)] {
        let (rendered, asked) =
            render_from(&map(LONGLAT, MAP_EXTENT, layer_projection), in_longlat());
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        assert_square_drawn(&rendered);
        assert_eq!(
            asked,
            [MAP_EXTENT],
            "{layer_projection:?}: the map's extent as it is"
        );
    }
    // A map without a projection reprojects no layer: the square's metres lie far off its view.
    let mut unprojected = map(LONGLAT, MAP_EXTENT, Some(LCC));
    unprojected.projection = None;
    let in_lcc = Recording::new(vec![Geometry::Polygon(vec![square_in(LCC)])]);
    let (rendered, asked) = render_from(&unprojected, in_lcc);
    assert_eq!(asked, [MAP_EXTENT]);
    let image = rendered.image.unwrap();
    let (x, y) = pixel(-64.0, 45.0);
    assert_eq!(image.pixel(x, y), Some(WHITE));
}

#[test]
fn under_a_map_of_auto_a_layer_is_drawn_in_its_own_longitude_and_latitude() {
    // A map of PROJECTION AUTO has no PROJ object: a layer in UTM differs from it and is drawn in
    // the longitude and latitude of its own datum (msProjectCreateReprojector); a layer without a
    // projection is drawn as it is.
    const UTM_20: &str = "+proj=utm +zone=20 +datum=WGS84 +units=m +no_defs";
    let mut auto = map(LONGLAT, MAP_EXTENT, Some(UTM_20));
    (auto.projection, auto.projection_auto) = (None, true);
    let in_utm = Recording::new(vec![Geometry::Polygon(vec![square_in(UTM_20)])]);
    let (rendered, asked) = render_from(&auto, in_utm);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_square_drawn(&rendered);
    let [asked] = asked[..] else {
        panic!("one query, not {asked:?}");
    };
    assert!(
        asked.max_x < 1_000_000.0 && asked.min_y > 4_000_000.0,
        "in UTM's metres: {asked:?}"
    );
    auto.layers[0].projection = None;
    let in_longlat = Recording::new(vec![Geometry::Polygon(vec![square_in(LONGLAT)])]);
    let (rendered, asked) = render_from(&auto, in_longlat);
    assert_square_drawn(&rendered);
    assert_eq!(asked, [MAP_EXTENT]);
}

/// A map in `map_projection` over `extent`, its layer of `PROJECTION AUTO`.
fn auto_layer(map_projection: &str, extent: Extent) -> Map {
    let mut map = map(map_projection, extent, None);
    map.layers[0].projection = Some(LayerProjection::FromSource);
    map
}

#[test]
fn a_layer_of_auto_takes_the_projection_its_source_declares() {
    // Verifies: ADR-draft-declared-projection
    let declared = DeclaredProjection::Proj4(LCC.to_owned());
    let in_lcc = Recording::new(vec![Geometry::Polygon(vec![square_in(LCC)])]).declaring(declared);
    let (rendered, asked) = render_from(&auto_layer(LONGLAT, MAP_EXTENT), in_lcc);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_square_drawn(&rendered);
    assert!(
        asked[0].min_x > 1_000_000.0,
        "in the conic's metres: {asked:?}"
    );
    // No projection at all: nothing is reprojected.
    let in_longlat = || Recording::new(vec![Geometry::Polygon(vec![square_in(LONGLAT)])]);
    let none = in_longlat().declaring(DeclaredProjection::NoProjection);
    let (rendered, asked) = render_from(&auto_layer(LONGLAT, MAP_EXTENT), none);
    assert_square_drawn(&rendered);
    assert_eq!(asked, [MAP_EXTENT]);
}

#[test]
fn a_layer_of_auto_whose_source_declares_nothing_is_in_the_longitude_and_latitude_of_the_map() {
    // Verifies: ADR-draft-declared-projection
    // Under a map in the conic, its coordinates are taken as longitude and latitude in the map's
    // datum, as msProjectCreateReprojector takes an AUTO without a PROJ object.
    let extent = way(LONGLAT, LCC).extent(&MAP_EXTENT).unwrap();
    let in_longlat = Recording::new(vec![Geometry::Polygon(vec![square_in(LONGLAT)])]);
    let (rendered, asked) = render_from(&auto_layer(LCC, extent), in_longlat);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    let green = (0..101).flat_map(|x| (0..101).map(move |y| (x, y)));
    assert!(
        green
            .filter(|&(x, y)| image.pixel(x, y) == Some(GREEN))
            .count()
            > 100
    );
    assert!(
        asked[0].min_x < -60.0 && asked[0].max_y > 50.0,
        "in degrees: {asked:?}"
    );
    // One that does not read is treated alike, with a warning naming the layer and the file.
    let unreadable = DeclaredProjection::Unreadable {
        file: "data.prj".to_owned(),
        why: "the datum D_European_1950 is not supported".to_owned(),
    };
    let in_longlat = Recording::new(vec![Geometry::Polygon(vec![square_in(LONGLAT)])]);
    let (rendered, _) = render_from(&auto_layer(LCC, extent), in_longlat.declaring(unreadable));
    let [warning] = &rendered.diagnostics[..] else {
        panic!("one warning: {:?}", rendered.diagnostics);
    };
    assert_eq!(warning.code, "projection-unreadable");
    assert!(
        warning.message.contains("`data`") && warning.message.contains("data.prj"),
        "{}",
        warning.message
    );
    // Under a map in longitude and latitude, nothing is reprojected.
    let in_longlat = Recording::new(vec![Geometry::Polygon(vec![square_in(LONGLAT)])]);
    let (rendered, asked) = render_from(&auto_layer(LONGLAT, MAP_EXTENT), in_longlat);
    assert_square_drawn(&rendered);
    assert_eq!(asked, [MAP_EXTENT]);
}

#[test]
fn lines_mapserver_cuts_with_geos_are_drawn_uncut_with_a_warning() {
    // Verifies: ADR-0037
    let polar = "+proj=ortho +lat_0=90 +lon_0=0 +datum=WGS84";
    let mut map = map(
        LONGLAT,
        Extent {
            min_x: -180.0,
            min_y: 0.0,
            max_x: 180.0,
            max_y: 90.0,
        },
        Some(polar),
    );
    map.layers[0].render_mode = Some(RenderMode::Line);
    let line = vec![
        Coord {
            x: -1_000_000.0,
            y: 10.0,
        },
        Coord {
            x: 1_000_000.0,
            y: 10.0,
        },
    ];
    let (rendered, _) = render_from(&map, Recording::new(vec![Geometry::LineString(line)]));
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert!(codes.contains(&"lines-not-cut"), "{codes:?}");
    // A polygon layer is not cut by GEOS, and is not warned of.
    map.layers[0].render_mode = Some(RenderMode::Polygon);
    let ring = vec![
        Coord { x: -1e6, y: -1e6 },
        Coord { x: 1e6, y: -1e6 },
        Coord { x: 1e6, y: 1e6 },
        Coord { x: -1e6, y: -1e6 },
    ];
    let (rendered, _) = render_from(&map, Recording::new(vec![Geometry::Polygon(vec![ring])]));
    assert!(
        rendered
            .diagnostics
            .iter()
            .all(|d| d.code != "lines-not-cut")
    );
}

#[test]
fn the_source_is_asked_for_the_maps_extent_in_its_own_coordinates() {
    // Verifies: ADR-0037
    let source = Recording::new(Vec::new());
    let (_, asked) = render_from(&map(LONGLAT, MAP_EXTENT, Some(LCC)), source);
    let [asked] = asked[..] else {
        panic!("one query, not {asked:?}");
    };
    let to_lcc = way(LONGLAT, LCC);
    for i in 0..=10 {
        let t = f64::from(i) / 10.0;
        let lon = MAP_EXTENT.min_x + t * (MAP_EXTENT.max_x - MAP_EXTENT.min_x);
        let lat = MAP_EXTENT.min_y + t * (MAP_EXTENT.max_y - MAP_EXTENT.min_y);
        for (x, y) in [
            (lon, MAP_EXTENT.min_y),
            (lon, MAP_EXTENT.max_y),
            (MAP_EXTENT.min_x, lat),
            (MAP_EXTENT.max_x, lat),
            (-65.0, 45.0),
        ] {
            let p = to_lcc.point(Coord { x, y }).unwrap();
            assert!(
                asked.min_x <= p.x
                    && p.x <= asked.max_x
                    && asked.min_y <= p.y
                    && p.y <= asked.max_y,
                "{x} {y} at {p:?} lies outside the area asked, {asked:?}"
            );
        }
    }
    // In metres of the conic, east of its central meridian at 95°W.
    assert!(asked.min_x > 1_000_000.0, "{asked:?}");
}

#[test]
fn a_map_around_a_pole_asks_its_source_for_every_longitude_as_mapserver_asks_its_first() {
    // Verifies: ADR-0037
    // A conic whose origin is the north pole, as msProjectRect detects polar stereographic: the
    // pole lies in the map's extent, so the source is asked for all longitudes up to the pole.
    // MapServer does so for the first layer of a render only, as it then remembers the map's
    // projection as polar (crates/proj/src/rect.rs); here every layer is asked as the first.
    let polar = "+proj=lcc +lat_1=70 +lat_2=85 +lat_0=90 +lon_0=0 +x_0=0 +y_0=0 +datum=WGS84 \
                 +units=m +no_defs";
    let around = Extent {
        min_x: -1_000_000.0,
        min_y: -1_000_000.0,
        max_x: 1_000_000.0,
        max_y: 1_000_000.0,
    };
    let (_, asked) = render_from(
        &map(polar, around, Some(LONGLAT)),
        Recording::new(Vec::new()),
    );
    let [asked] = asked[..] else {
        panic!("one query, not {asked:?}");
    };
    assert_eq!(
        (asked.min_x, asked.max_x, asked.max_y),
        (-180.0, 180.0, 90.0)
    );
    assert!(70.0 < asked.min_y && asked.min_y < 80.0, "{asked:?}");
}

#[test]
fn a_point_that_does_not_reproject_is_left_out_with_a_warning_naming_the_layer() {
    // Verifies: ADR-0037, Q-3
    // A map in the conic, a layer in longitude and latitude: the south pole, which the conic
    // opens away from, has no place on it.
    let extent = way(LONGLAT, LCC).extent(&MAP_EXTENT).unwrap();
    let mut layer_map = map(LCC, extent, Some(LONGLAT));
    layer_map.layers[0].render_mode = Some(RenderMode::Point);
    let pole = Coord { x: 0.0, y: -90.0 };
    let island = Coord { x: -63.0, y: 46.4 };
    let source = Recording::new(vec![
        Geometry::Point(pole),
        Geometry::MultiPoint(vec![island, pole]),
        Geometry::LineString(vec![island, pole]),
    ]);
    let (rendered, _) = render_from(&layer_map, source);
    assert!(rendered.image.is_some());
    let warnings: Vec<_> = rendered
        .diagnostics
        .iter()
        .filter(|d| d.code == "points-not-reprojected")
        .collect();
    assert_eq!(warnings.len(), 1, "{:?}", rendered.diagnostics);
    assert!(
        warnings[0].message.contains("layer `data`: 3 points"),
        "{}",
        warnings[0].message
    );
}

#[test]
fn a_projection_not_supported_leaves_its_layer_undrawn_with_a_warning() {
    // Verifies: ADR-0037
    let source = Recording::new(vec![Geometry::Polygon(vec![square_in(LONGLAT)])]);
    let (rendered, asked) = render_from(
        &map(
            LONGLAT,
            MAP_EXTENT,
            Some("+proj=omerc +lat_0=45 +lonc=-64 +alpha=30 +ellps=GRS80"),
        ),
        source,
    );
    assert!(asked.is_empty(), "its source is not read");
    let codes: Vec<_> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["projection-unsupported"]);
    assert!(
        rendered.diagnostics[0].message.contains("layer `data`")
            && rendered.diagnostics[0].message.contains("+proj=omerc"),
        "{}",
        rendered.diagnostics[0].message
    );
    let image = rendered.image.unwrap();
    let (x, y) = pixel(-64.0, 45.0);
    assert_eq!(image.pixel(x, y), Some(WHITE));
}

#[test]
fn inline_features_in_another_projection_are_reprojected() {
    // Verifies: ADR-0037, ADR-0018
    let mut inline_map = map(LONGLAT, MAP_EXTENT, Some(LCC));
    inline_map.layers[0].features = vec![InlineFeature {
        shape_type: ShapeType::Polygon,
        parts: vec![square_in(LCC).iter().map(|c| [c.x, c.y]).collect()],
        properties: Vec::new(),
        text: None,
        span: None,
    }];
    let rendered = ready(render(
        &inline_map,
        &BTreeMap::<String, Recording>::new(),
        AggRenderer::new(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_square_drawn(&rendered);
}
