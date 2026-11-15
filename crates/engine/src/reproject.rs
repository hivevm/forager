//! The reprojection of a layer's features into the map's projection, as `msDrawShape` reprojects
//! each shape before it draws it, and of the area asked of the layer's source into the layer's.

use forager_ports::{Coord, Feature, Geometry};
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

/// How `layer` is reprojected into `map`'s projection: not where either has no projection, where
/// they do not differ as `msProjectionsDiffer` finds it, or where the layer's comes from its
/// source, which no source declares yet; an error where either does not read. Under a map of
/// `AUTO`, which has no PROJ object, a layer in a projection differs from it and is reprojected
/// into its own base geographic CRS, as `msProjectCreateReprojector` reprojects it.
pub(crate) fn for_layer(map: &Map, layer: &Layer) -> Result<Option<LayerReprojection>, Error> {
    let Some(LayerProjection::Proj4(layer_projection)) = &layer.projection else {
        return Ok(None);
    };
    let to_map = if map.projection_auto {
        if layer_projection.split_whitespace().next().is_none() {
            return Ok(None);
        }
        Reprojection::into_auto(Projection::parse(layer_projection)?)
    } else {
        let Some(map_projection) = &map.projection else {
            return Ok(None);
        };
        if !differ(layer_projection, map_projection) {
            return Ok(None);
        }
        Reprojection::new(
            Projection::parse(layer_projection)?,
            Projection::parse(map_projection)?,
        )
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
    /// shape, and returns how many of their points do not reproject. A line or a ring is
    /// reprojected by `msProjectShapeLine`: a run of points that do not reproject is cut at the
    /// horizon, a line into parts added after the shape's others, and a longitude into longitude
    /// and latitude is wrapped across the dateline. The points of a point shape are reprojected by
    /// `msProjectLineEx`, all of them left out where one does not reproject, unless the map is in
    /// longitude and latitude and the layer is not. A line left empty is left out, and a feature
    /// left with nothing.
    pub(crate) fn features(&self, features: &mut Vec<Feature>) -> Reprojected {
        self.features_within(features, MAX_HORIZON_POINTS)
    }

    /// [`Self::features`], the points the horizon search adds held to `bound`: the feature that
    /// passes it and those after it are left out.
    fn features_within(&self, features: &mut Vec<Feature>, bound: usize) -> Reprojected {
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

    /// The lines of one shape, each reprojected by `msProjectShapeLine`, last first as
    /// `msProjectShapeEx` takes them: each in its place, the lines they add after them all, and
    /// those left empty left out.
    fn lines(&self, lines: Vec<Vec<Coord>>, polygon: bool, failed: &mut Count) -> Vec<Vec<Coord>> {
        let mut placed = Vec::with_capacity(lines.len());
        let mut added = Vec::new();
        for line in lines.iter().rev() {
            let reprojected = self.to_map.shape_line(line, polygon);
            failed.failed += reprojected.failed;
            failed.horizon += reprojected.horizon_points;
            placed.push(reprojected.first);
            added.extend(reprojected.added);
        }
        placed.reverse();
        placed.extend(added);
        placed.retain(|line| !line.is_empty());
        placed
    }

    /// The rings of a multipolygon, one shape to MapServer, each in its place and the lines they
    /// add with the last polygon; whether any is left.
    fn polygons(&self, polygons: &mut Vec<Vec<Vec<Coord>>>, failed: &mut Count) -> bool {
        let mut added = Vec::new();
        for rings in polygons.iter_mut().rev() {
            for ring in rings.iter_mut().rev() {
                let reprojected = self.to_map.shape_line(ring, true);
                failed.failed += reprojected.failed;
                failed.horizon += reprojected.horizon_points;
                *ring = reprojected.first;
                added.extend(reprojected.added);
            }
        }
        if let Some(last) = polygons.last_mut() {
            last.extend(added);
        }
        for rings in polygons.iter_mut() {
            rings.retain(|ring| !ring.is_empty());
        }
        polygons.retain(|rings| !rings.is_empty());
        !polygons.is_empty()
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
        for_layer(&map, &layer).unwrap().unwrap()
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
        assert_eq!(into_the_conic().features(&mut features).failed, 1);
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
        assert_eq!(into_the_conic().features(&mut features).failed, 4);
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
        assert_eq!(into_the_conic().features(&mut points).failed, 1);
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
}
