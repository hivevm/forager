//! The reprojection of a layer's features into the map's projection, as `msDrawShape` reprojects
//! each shape before it draws it, and of the area asked of the layer's source into the layer's.

use forager_ports::{Coord, Feature, Geometry};
use forager_proj::{Error, Projection, Reprojection, differ};
use forager_style_ir::{Extent, Layer, LayerProjection, Map};

/// The ways between a layer's projection and the map's.
pub(crate) struct LayerReprojection {
    /// From the layer's projection into the map's, for its features.
    to_map: Reprojection,
    /// From the map's projection into the layer's, for the area asked of its source.
    to_layer: Reprojection,
}

/// How `layer` is reprojected into `map`'s projection: not where either has no projection, where
/// they do not differ as `msProjectionsDiffer` finds it, or where the layer's comes from its
/// source, which no source declares yet; an error where either does not read.
pub(crate) fn for_layer(map: &Map, layer: &Layer) -> Result<Option<LayerReprojection>, Error> {
    let (Some(map_projection), Some(LayerProjection::Proj4(layer_projection))) =
        (&map.projection, &layer.projection)
    else {
        return Ok(None);
    };
    if !differ(layer_projection, map_projection) {
        return Ok(None);
    }
    let to_map = Reprojection::new(
        Projection::parse(layer_projection)?,
        Projection::parse(map_projection)?,
    );
    Ok(Some(LayerReprojection {
        to_layer: to_map.reversed(),
        to_map,
    }))
}

impl LayerReprojection {
    /// The area the layer's source is asked for: the map's `extent` reprojected into the layer's
    /// projection, its sides sampled; `None` where no point of them reprojects.
    pub(crate) fn asked(&self, extent: &Extent) -> Option<Extent> {
        self.to_layer.extent(extent)
    }

    /// Reprojects each of `features` into the map's projection, and returns how many of their
    /// points do not reproject. Those are left out; a line left with fewer than two points, a ring
    /// with fewer than three, a polygon whose exterior ring is left out, so that a hole never
    /// stands first, and a part, a geometry, or a feature left with nothing are left out with
    /// them.
    pub(crate) fn features(&self, features: &mut Vec<Feature>) -> usize {
        let mut left_out = 0;
        features.retain_mut(|feature| self.geometry(&mut feature.geometry, &mut left_out));
        left_out
    }

    /// Reprojects `geometry`; whether anything of it is left. Counts into `left_out` the points
    /// that do not reproject.
    fn geometry(&self, geometry: &mut Geometry, left_out: &mut usize) -> bool {
        match geometry {
            Geometry::Point(point) => match self.to_map.point(*point) {
                Ok(at) => {
                    *point = at;
                    true
                }
                Err(_) => {
                    *left_out += 1;
                    false
                }
            },
            Geometry::MultiPoint(points) => self.points(points, left_out, 1),
            Geometry::LineString(line) => self.points(line, left_out, 2),
            Geometry::MultiLineString(lines) => self.parts(lines, left_out, 2),
            Geometry::Polygon(rings) => self.polygon(rings, left_out),
            Geometry::MultiPolygon(polygons) => {
                polygons.retain_mut(|rings| self.polygon(rings, left_out));
                !polygons.is_empty()
            }
            Geometry::Collection(members) => {
                members.retain_mut(|member| self.geometry(member, left_out));
                !members.is_empty()
            }
        }
    }

    /// Reprojects a polygon's `rings`, the exterior first; whether its exterior ring is left, its
    /// holes then kept where each keeps three points.
    fn polygon(&self, rings: &mut Vec<Vec<Coord>>, left_out: &mut usize) -> bool {
        let Some(exterior) = rings.first_mut() else {
            return false;
        };
        if !self.points(exterior, left_out, 3) {
            return false;
        }
        let mut first = true;
        rings.retain_mut(|ring| std::mem::take(&mut first) || self.points(ring, left_out, 3));
        true
    }

    /// Reprojects each of `parts`, a line's or a ring's points; whether any keeps `least`.
    fn parts(&self, parts: &mut Vec<Vec<Coord>>, left_out: &mut usize, least: usize) -> bool {
        parts.retain_mut(|part| self.points(part, left_out, least));
        !parts.is_empty()
    }

    /// Reprojects `points`, leaving out those that do not reproject; whether `least` are left.
    fn points(&self, points: &mut Vec<Coord>, left_out: &mut usize, least: usize) -> bool {
        points.retain_mut(|point| match self.to_map.point(*point) {
            Ok(at) => {
                *point = at;
                true
            }
            Err(_) => {
                *left_out += 1;
                false
            }
        });
        points.len() >= least
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

    const SQUARE: &[(f64, f64)] = &[(-64.0, 45.0), (-62.0, 45.0), (-62.0, 47.0), (-64.0, 45.0)];
    const HOLE: &[(f64, f64)] = &[(-63.5, 45.5), (-62.5, 45.5), (-63.0, 46.0), (-63.5, 45.5)];
    const THROUGH_THE_POLE: &[(f64, f64)] =
        &[(-64.0, 45.0), (0.0, -90.0), (-62.0, 47.0), (-64.0, 45.0)];

    #[test]
    fn a_polygon_whose_exterior_ring_does_not_reproject_is_left_out_with_its_holes() {
        // Verifies: ADR-draft-reprojection-ported-in-proj
        // Left in, the hole would stand first and be drawn as the exterior.
        let mut features = vec![polygon(vec![
            ring(&[(-64.0, 45.0), (0.0, -90.0), (-62.0, 47.0)]),
            ring(HOLE),
        ])];
        assert_eq!(into_the_conic().features(&mut features), 1);
        assert!(features.is_empty());
    }

    #[test]
    fn a_hole_that_does_not_reproject_leaves_its_polygon_drawn_without_it() {
        // Verifies: ADR-draft-reprojection-ported-in-proj
        let mut features = vec![polygon(vec![
            ring(SQUARE),
            ring(&[(-63.5, 45.5), (0.0, -90.0), (-63.5, 45.5)]),
            ring(HOLE),
        ])];
        assert_eq!(into_the_conic().features(&mut features), 1);
        let [
            Feature {
                geometry: Geometry::Polygon(rings),
                ..
            },
        ] = &features[..]
        else {
            panic!("one polygon left: {features:?}");
        };
        assert_eq!(rings.len(), 2, "the exterior and the hole that reprojects");
        assert!(
            rings[0][0].x > 2_000_000.0,
            "in metres of the conic: {:?}",
            rings[0][0]
        );
        // A ring that keeps three points after one is left out stays.
        let mut kept = vec![polygon(vec![ring(THROUGH_THE_POLE)])];
        assert_eq!(into_the_conic().features(&mut kept), 1);
        assert_eq!(kept.len(), 1);
    }
}
