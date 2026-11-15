//! The reprojection of a layer's features into the map's projection, as `msDrawShape` reprojects
//! each shape before it draws it, and of the area asked of the layer's source into the layer's.

use forager_ports::{Coord, DeclaredProjection, Feature, Geometry};
use forager_proj::{Error, Projection, Reprojection, differ};
use forager_style_ir::{Extent, Layer, LayerProjection, Map};

/// The most points the horizon search of one layer's reprojection adds to the shapes of its
/// features (Q-9); the layer stops there with a diagnostic.
pub const MAX_HORIZON_POINTS: usize = 1_000_000;

/// What reprojecting a layer's features did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Reprojected {
    /// The points that did not reproject.
    pub(crate) failed: usize,
    /// The points the horizon search added.
    pub(crate) horizon_points: usize,
    /// Whether the horizon points passed [`MAX_HORIZON_POINTS`], and the features from the one
    /// that passed it on were left out.
    pub(crate) stopped: bool,
}

/// The points of one feature that did not reproject, and those the horizon search added.
#[derive(Default)]
struct Count {
    failed: usize,
    horizon: usize,
}

/// The ways between a layer's projection and the map's.
pub(crate) struct LayerReprojection {
    /// From the layer's projection into the map's, for its features.
    to_map: Reprojection,
    /// From the map's projection into the layer's, for the area asked of its source.
    to_layer: Reprojection,
}

/// A side of a reprojection as MapServer holds it.
enum Side<'a> {
    /// No parameters: nothing differs from it.
    Empty,
    /// `AUTO` without a PROJ object.
    Auto,
    /// A proj4 string.
    Proj4(&'a str),
}

/// How `layer` is reprojected into `map`'s projection, `declared` the projection its source
/// declares for a layer of `AUTO` (ADR-0039): not where either has no
/// parameters or they do not differ as `msProjectionsDiffer` finds it; an error where either does
/// not read. A side of `AUTO` without a PROJ object is taken as longitude and latitude in the
/// other's base geographic CRS, as `msProjectCreateReprojector` takes it, and nothing is
/// reprojected where that is the other side itself.
pub(crate) fn for_layer(
    map: &Map,
    layer: &Layer,
    declared: &DeclaredProjection,
) -> Result<Option<LayerReprojection>, Error> {
    let layer_side = match &layer.projection {
        None => Side::Empty,
        Some(LayerProjection::Proj4(p)) => Side::Proj4(p),
        Some(LayerProjection::FromSource) => match declared {
            DeclaredProjection::Proj4(p) => Side::Proj4(p),
            DeclaredProjection::NoProjection => Side::Empty,
            DeclaredProjection::None | DeclaredProjection::Unreadable { .. } => Side::Auto,
        },
    };
    let map_side = match (&map.projection, map.projection_auto) {
        (_, true) => Side::Auto,
        (Some(p), false) => Side::Proj4(p),
        (None, false) => Side::Empty,
    };
    let empty = |p: &str| p.split_whitespace().next().is_none();
    let to_map = match (layer_side, map_side) {
        (Side::Empty, _) | (_, Side::Empty) | (Side::Auto, Side::Auto) => return Ok(None),
        (Side::Proj4(l), _) if empty(l) => return Ok(None),
        (_, Side::Proj4(m)) if empty(m) => return Ok(None),
        (Side::Proj4(l), Side::Auto) => Reprojection::into_auto(Projection::parse(l)?),
        (Side::Auto, Side::Proj4(m)) => {
            let map = Projection::parse(m)?;
            // Longitude and latitude into its own geographic CRS: MapServer does nothing.
            if map.is_geographic() {
                return Ok(None);
            }
            Reprojection::into_auto(map).reversed()
        }
        (Side::Proj4(l), Side::Proj4(m)) => {
            if !differ(l, m) {
                return Ok(None);
            }
            Reprojection::new(Projection::parse(l)?, Projection::parse(m)?)
        }
    };
    Ok(Some(LayerReprojection {
        to_layer: to_map.reversed(),
        to_map,
    }))
}

impl LayerReprojection {
    /// The area the layer's source is asked for: the map's `extent` reprojected into the layer's
    /// projection as `msProjectRect` reprojects it; `None` where no point of it reprojects.
    pub(crate) fn asked(&self, extent: &Extent) -> Option<Extent> {
        self.to_layer.extent(extent)
    }

    /// Reprojects each of `features` into the map's projection as `msProjectShapeEx` reprojects a
    /// shape, and returns how many of their points do not reproject and how many points the
    /// horizon search added, those past `bound` leaving out the feature that passes it and the
    /// features after it (Q-9). A line or a ring is
    /// reprojected by `msProjectShapeLine`: a run of points that do not reproject is cut at the
    /// horizon, a line into parts added after the shape's others, and a longitude into longitude
    /// and latitude is wrapped across the dateline. The points of a point shape are reprojected by
    /// `msProjectLineEx`, all of them left out where one does not reproject, unless the map is in
    /// longitude and latitude and the layer is not. A line left empty is left out, and a feature
    /// left with nothing.
    pub(crate) fn features_within(&self, features: &mut Vec<Feature>, bound: usize) -> Reprojected {
        let mut done = Reprojected::default();
        let mut kept = 0;
        for i in 0..features.len() {
            let mut count = Count::default();
            let left = self.geometry(&mut features[i].geometry, &mut count);
            done.failed += count.failed;
            done.horizon_points += count.horizon;
            if done.horizon_points > bound {
                done.stopped = true;
                break;
            }
            if left {
                features.swap(kept, i);
                kept += 1;
            }
        }
        features.truncate(kept);
        done
    }

    /// Reprojects `geometry`; whether anything of it is left. Counts into `failed` the points that
    /// do not reproject.
    fn geometry(&self, geometry: &mut Geometry, failed: &mut Count) -> bool {
        match geometry {
            Geometry::Point(point) => {
                let mut points = [*point];
                let kept = self.point_line(&mut points, failed);
                *point = points[0];
                kept
            }
            Geometry::MultiPoint(points) => self.point_line(points, failed),
            Geometry::LineString(line) => {
                let mut lines = self.lines(vec![std::mem::take(line)], false, failed);
                match lines.len() {
                    0 => false,
                    1 => {
                        *line = lines.remove(0);
                        true
                    }
                    _ => {
                        *geometry = Geometry::MultiLineString(lines);
                        true
                    }
                }
            }
            Geometry::MultiLineString(lines) => {
                *lines = self.lines(std::mem::take(lines), false, failed);
                !lines.is_empty()
            }
            Geometry::Polygon(rings) => {
                *rings = self.lines(std::mem::take(rings), true, failed);
                !rings.is_empty()
            }
            Geometry::MultiPolygon(polygons) => self.polygons(polygons, failed),
            Geometry::Collection(members) => {
                members.retain_mut(|member| self.geometry(member, failed));
                !members.is_empty()
            }
        }
    }

    /// `msProjectLineEx` of a point shape's `points`; whether they are kept.
    fn point_line(&self, points: &mut [Coord], failed: &mut Count) -> bool {
        match self.to_map.point_line(points) {
            Ok(count) => {
                failed.failed += count;
                true
            }
            Err(count) => {
                failed.failed += count;
                false
            }
        }
    }

    /// The lines of one shape reprojected by [`Reprojection::shape`], as `msProjectShapeEx`
    /// leaves them, those left empty left out.
    fn lines(&self, lines: Vec<Vec<Coord>>, polygon: bool, failed: &mut Count) -> Vec<Vec<Coord>> {
        let shape = self.to_map.shape(&lines, polygon);
        failed.failed += shape.failed;
        failed.horizon += shape.horizon_points;
        let mut lines = shape.lines;
        lines.retain(|line| !line.is_empty());
        lines
    }

    /// The polygons of a multipolygon, each reprojected as a shape of its own, so that a ring
    /// moved after the others stays with its holes: MapServer holds them in one shape and moves
    /// such a ring after all of them, which an even-odd fill over the whole shape draws alike, but
    /// the engine fills each polygon apart. Whether any is left.
    fn polygons(&self, polygons: &mut Vec<Vec<Vec<Coord>>>, failed: &mut Count) -> bool {
        for rings in polygons.iter_mut() {
            *rings = self.lines(std::mem::take(rings), true, failed);
        }
        polygons.retain(|rings| !rings.is_empty());
        !polygons.is_empty()
    }

    /// Whether MapServer would cut the lines of this layer's line shapes with GEOS, which the
    /// engine does not ([`Reprojection::cuts_lines_with_geos`]).
    pub(crate) fn cuts_lines_with_geos(&self) -> bool {
        self.to_map.cuts_lines_with_geos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forager_ports::Properties;

    /// The way from longitude and latitude into EPSG:3978, whose cone has no place for the south
    /// pole.
    fn into_the_conic() -> LayerReprojection {
        let map = Map {
            projection: Some(
                "+proj=lcc +lat_1=49 +lat_2=77 +lat_0=49 +lon_0=-95 +ellps=GRS80 +units=m".into(),
            ),
            ..Map::default()
        };
        let layer = Layer {
            projection: Some(LayerProjection::Proj4("+proj=longlat +datum=WGS84".into())),
            ..Layer::default()
        };
        for_layer(&map, &layer, &DeclaredProjection::None)
            .unwrap()
            .unwrap()
    }

    fn ring(points: &[(f64, f64)]) -> Vec<Coord> {
        points.iter().map(|&(x, y)| Coord { x, y }).collect()
    }

    fn polygon(rings: Vec<Vec<Coord>>) -> Feature {
        Feature {
            id: 0,
            geometry: Geometry::Polygon(rings),
            properties: Properties::default(),
        }
    }

    const HOLE: &[(f64, f64)] = &[(-63.5, 45.5), (-62.5, 45.5), (-63.0, 46.0), (-63.5, 45.5)];

    #[test]
    fn a_ring_through_a_point_that_does_not_reproject_is_cut_at_the_horizon() {
        // Verifies: ADR-0037
        // The conic has no place for the south pole: msProjectShapeLine halves the sides to it
        // down to the horizon, and the ring keeps its place, its hole after it.
        let mut features = vec![polygon(vec![
            ring(&[
                (-64.0, 45.0),
                (-62.0, 45.0),
                (-62.0, 47.0),
                (-64.0, 47.0),
                (-90.0, -90.0),
                (-64.0, 45.0),
            ]),
            ring(HOLE),
        ])];
        assert_eq!(
            into_the_conic()
                .features_within(&mut features, MAX_HORIZON_POINTS)
                .failed,
            1
        );
        let [
            Feature {
                geometry: Geometry::Polygon(rings),
                ..
            },
        ] = &features[..]
        else {
            panic!("one polygon left: {features:?}");
        };
        assert_eq!(rings.len(), 2, "{rings:?}");
        // A run of no more than three such points moves the ring's points after the shape's other
        // lines, as MapServer moves them: the hole stands first, which an even-odd fill draws
        // alike. The ring holds its four points, a horizon point down each side to the pole, and
        // the first closing it.
        assert_eq!(rings[0].len(), HOLE.len());
        let exterior = &rings[1];
        assert_eq!(exterior.len(), 7, "{exterior:?}");
        assert_eq!(exterior[0], exterior[6]);
        assert!(
            exterior[0].x > 2_000_000.0,
            "in metres of the conic: {:?}",
            exterior[0]
        );
    }

    #[test]
    fn a_line_that_leaves_the_projection_is_cut_into_parts() {
        // Verifies: ADR-0037
        let line = |points: &[(f64, f64)]| Feature {
            id: 0,
            geometry: Geometry::LineString(ring(points)),
            properties: Properties::default(),
        };
        let mut features = vec![line(&[
            (-64.0, 45.0),
            (-62.0, 45.0),
            (-90.0, -90.0),
            (-90.0, -90.0),
            (-90.0, -90.0),
            (-90.0, -90.0),
            (-62.0, 47.0),
            (-64.0, 47.0),
        ])];
        assert_eq!(
            into_the_conic()
                .features_within(&mut features, MAX_HORIZON_POINTS)
                .failed,
            4
        );
        let Geometry::MultiLineString(parts) = &features[0].geometry else {
            panic!("two parts: {features:?}");
        };
        // Each part ends, or starts, at the horizon.
        assert_eq!(parts.iter().map(Vec::len).collect::<Vec<_>>(), [3, 3]);
        // A point shape loses all its points where one does not reproject.
        let mut points = vec![Feature {
            id: 0,
            geometry: Geometry::MultiPoint(ring(&[(-64.0, 45.0), (0.0, -90.0)])),
            properties: Properties::default(),
        }];
        assert_eq!(
            into_the_conic()
                .features_within(&mut points, MAX_HORIZON_POINTS)
                .failed,
            1
        );
        assert!(points.is_empty());
    }

    #[test]
    fn a_layer_whose_horizon_points_pass_the_bound_stops_at_the_feature_that_passes_it() {
        // Verifies: Q-9
        // Each line leaves the conic for the south pole and comes back: a horizon point each way.
        let leaving = || Feature {
            id: 0,
            geometry: Geometry::LineString(ring(&[
                (-64.0, 45.0),
                (-90.0, -90.0),
                (-90.0, -90.0),
                (-90.0, -90.0),
                (-90.0, -90.0),
                (-62.0, 47.0),
            ])),
            properties: Properties::default(),
        };
        let mut features: Vec<Feature> = (0..5).map(|_| leaving()).collect();
        let done = into_the_conic().features_within(&mut features, 5);
        assert!(done.stopped);
        assert_eq!(
            features.len(),
            2,
            "the two before the third, which passes 5"
        );
        let mut features: Vec<Feature> = (0..5).map(|_| leaving()).collect();
        let done = into_the_conic().features_within(&mut features, 10);
        assert_eq!(
            (done.stopped, done.horizon_points, features.len()),
            (false, 10, 5)
        );
        assert_eq!(MAX_HORIZON_POINTS, 1_000_000);
    }

    #[test]
    fn the_warning_of_the_horizon_bound_names_the_layer_and_q_9() {
        // Verifies: Q-9
        let layer = Layer {
            name: Some("coast".into()),
            ..Layer::default()
        };
        // The render warns once the bound is passed, and draws the features before.
        let leaving = Feature {
            id: 0,
            geometry: Geometry::LineString(ring(&[(-64.0, 45.0), (-90.0, -90.0), (-62.0, 47.0)])),
            properties: Properties::default(),
        };
        let mut features = vec![leaving.clone(), leaving.clone(), leaving];
        let mut diagnostics = Vec::new();
        crate::reproject_features_within(
            &into_the_conic(),
            &layer,
            &mut features,
            &mut diagnostics,
            3,
        );
        let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["points-not-reprojected", "horizon-points-limit"]);
        assert_eq!(features.len(), 1);
        let warning = &diagnostics[1];
        assert!(
            warning.message.contains("`coast`")
                && warning.message.contains("1000000")
                && warning.message.contains("(Q-9)"),
            "{}",
            warning.message
        );
    }

    #[test]
    fn a_ring_moved_after_a_short_run_stays_with_its_own_polygon() {
        // Verifies: ADR-0037
        let through_the_pole = ring(&[
            (-64.0, 45.0),
            (-62.0, 45.0),
            (-62.0, 47.0),
            (-64.0, 47.0),
            (-90.0, -90.0),
            (-64.0, 45.0),
        ]);
        let other = ring(&[(-60.0, 45.0), (-59.0, 45.0), (-59.0, 46.0), (-60.0, 45.0)]);
        let mut features = vec![Feature {
            id: 0,
            geometry: Geometry::MultiPolygon(vec![vec![through_the_pole, ring(HOLE)], vec![other]]),
            properties: Properties::default(),
        }];
        assert_eq!(
            into_the_conic()
                .features_within(&mut features, MAX_HORIZON_POINTS)
                .failed,
            1
        );
        let Geometry::MultiPolygon(polygons) = &features[0].geometry else {
            panic!("{features:?}");
        };
        let counts: Vec<Vec<usize>> = polygons
            .iter()
            .map(|rings| rings.iter().map(Vec::len).collect())
            .collect();
        // The first polygon's exterior moves after its own hole, not into the last polygon.
        assert_eq!(counts, [vec![HOLE.len(), 7], vec![4]]);
    }

    #[test]
    fn a_polar_layer_into_longitude_and_latitude_is_one_mapserver_cuts_with_geos() {
        // Verifies: ADR-0037
        let way = |layer: &str| {
            let map = Map {
                projection: Some("+proj=longlat +datum=WGS84".into()),
                ..Map::default()
            };
            let layer = Layer {
                projection: Some(LayerProjection::Proj4(layer.into())),
                ..Layer::default()
            };
            for_layer(&map, &layer, &DeclaredProjection::None)
                .unwrap()
                .unwrap()
        };
        assert!(way("+proj=ortho +lat_0=90 +datum=WGS84").cuts_lines_with_geos());
        assert!(!way("+proj=ortho +lat_0=0 +datum=WGS84").cuts_lines_with_geos());
        assert!(!into_the_conic().cuts_lines_with_geos());
    }

    /// The polygons of the G-11 reference shapes, with their source and target projections.
    fn reference_polygons() -> Vec<(String, String, Vec<Vec<Coord>>)> {
        let point = |xy: &str| {
            let (x, y) = xy.split_once(',').unwrap();
            Coord {
                x: x.parse().unwrap(),
                y: y.parse().unwrap(),
            }
        };
        include_str!("../../proj/tests/reference/shapes.tsv")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .filter_map(|line| {
                let f: Vec<&str> = line.split('\t').collect();
                (f[2] == "polygon").then(|| {
                    let rings = f[3]
                        .split('|')
                        .map(|ring| ring.split(' ').map(point).collect())
                        .collect();
                    (f[0].to_owned(), f[1].to_owned(), rings)
                })
            })
            .collect()
    }

    #[test]
    fn a_reprojected_polygon_keeps_its_rings_beside_another_polygon() {
        // Verifies: ADR-0037
        // The engine fills each polygon of a multipolygon apart, so the rings MapServer moves after
        // a shape's others stay with the polygon they came from: each polygon of the reference
        // shapes, beside a ring that reprojects whole, keeps the rings proj's shape gives it.
        let polygons = reference_polygons();
        assert!(polygons.len() >= 4, "{} polygons", polygons.len());
        for (source, target, rings) in polygons {
            let map = Map {
                projection: Some(target.clone()),
                ..Map::default()
            };
            let layer = Layer {
                projection: Some(LayerProjection::Proj4(source.clone())),
                ..Layer::default()
            };
            let way = for_layer(&map, &layer, &DeclaredProjection::None)
                .unwrap()
                .unwrap();
            // A small ring about a point of the polygon that reprojects.
            let at = rings[0]
                .iter()
                .copied()
                .find(|&p| way.to_map.point(p).is_ok())
                .unwrap();
            let small = ring(&[
                (at.x, at.y),
                (at.x + 1e-3, at.y),
                (at.x + 1e-3, at.y + 1e-3),
                (at.x, at.y),
            ]);
            let mut expected = way.to_map.shape(&rings, true).lines;
            expected.retain(|r| !r.is_empty());
            let mut features = vec![Feature {
                id: 0,
                geometry: Geometry::MultiPolygon(vec![rings, vec![small]]),
                properties: Properties::default(),
            }];
            way.features_within(&mut features, MAX_HORIZON_POINTS);
            let Geometry::MultiPolygon(left) = &features[0].geometry else {
                panic!("{source} → {target}: {features:?}");
            };
            assert_eq!(left.len(), 2, "{source} → {target}: both polygons left");
            assert_eq!(
                left[0], expected,
                "{source} → {target}: the polygon's own rings"
            );
            assert_eq!(
                left[1].len(),
                1,
                "{source} → {target}: the small ring alone"
            );
        }
    }
}
