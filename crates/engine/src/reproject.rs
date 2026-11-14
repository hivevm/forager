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
    /// with fewer than three, and a part, a geometry, or a feature left with nothing are left out
    /// with them.
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
            Geometry::Polygon(rings) => self.parts(rings, left_out, 3),
            Geometry::MultiPolygon(polygons) => {
                polygons.retain_mut(|rings| self.parts(rings, left_out, 3));
                !polygons.is_empty()
            }
            Geometry::Collection(members) => {
                members.retain_mut(|member| self.geometry(member, left_out));
                !members.is_empty()
            }
        }
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
