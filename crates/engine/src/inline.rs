// The source follows MapServer's msINLINELayerNextShape and msLayerGetItemIndex, src/maplayer.c,
// of the release pinned in scripts/fetch-msautotest.sh.
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

//! The in-memory feature source of a layer's inline features (ADR-0033).

use std::borrow::Cow;
use std::rc::Rc;

use forager_ports::{
    Coord, Feature, FeatureQuery, FeatureSource, Features, Geometry, Properties, SourceError,
    Table, Value,
};
use forager_style_ir::{Extent, InlineFeature, Layer, ShapeType};

/// Whether `layer` is read from its inline features (ADR-0033): it has some, and its source is
/// not a graticule, which `msInitializeVirtualTable` (src/maplayer.c) exempts.
pub fn is_inline(layer: &Layer) -> bool {
    !layer.features.is_empty()
        && layer
            .source
            .as_ref()
            .is_none_or(|source| !source.kind.eq_ignore_ascii_case("graticule"))
}

/// The features of one layer, read from the style IR.
pub struct InlineSource<'a> {
    features: &'a [InlineFeature],
    table: Rc<Values>,
}

impl<'a> InlineSource<'a> {
    pub fn new(features: &'a [InlineFeature]) -> Self {
        let table = Values(
            features
                .iter()
                .map(|f| {
                    f.properties
                        .iter()
                        .map(|p| (p.name.clone(), p.value.clone()))
                        .collect()
                })
                .collect(),
        );
        Self {
            features,
            table: Rc::new(table),
        }
    }
}

impl FeatureSource for InlineSource<'_> {
    /// The features whose bounds meet the query's extent, in the order they were written, each
    /// with its place in that order as its id (msINLINELayerNextShape).
    async fn features(&self, query: &FeatureQuery) -> Result<Features, SourceError> {
        let features = self
            .features
            .iter()
            .zip(0u64..)
            .filter(|(f, _)| overlaps(&bounds(f), &query.extent))
            .map(|(f, id)| Feature {
                id,
                geometry: geometry(f),
                properties: Properties::record(self.table.clone(), id),
            })
            .collect::<Vec<_>>();
        Ok(features.into())
    }
}

/// The bounds `msComputeBounds` gives a shape: those of its points, or, of a shape without any,
/// the `-1` of each side that `msInitShape` leaves.
fn bounds(feature: &InlineFeature) -> Extent {
    let mut points = feature.parts.iter().flatten();
    let Some(&[x, y]) = points.next() else {
        return Extent {
            min_x: -1.0,
            min_y: -1.0,
            max_x: -1.0,
            max_y: -1.0,
        };
    };
    points.fold(
        Extent {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        },
        |e, &[x, y]| Extent {
            min_x: e.min_x.min(x),
            min_y: e.min_y.min(y),
            max_x: e.max_x.max(x),
            max_y: e.max_y.max(y),
        },
    )
}

/// `msRectOverlap`: whether two rectangles meet, their edges included.
fn overlaps(a: &Extent, b: &Extent) -> bool {
    !(a.min_x > b.max_x || a.max_x < b.min_x || a.min_y > b.max_y || a.max_y < b.min_y)
}

/// The geometry the port delivers of a shape, as the shapefile source delivers a record: a point
/// shape of one point as a point and any other as a multipoint of all its points, a line shape
/// of one part as a line string and any other as a multi-line string, and a polygon shape as one
/// polygon of all its parts.
fn geometry(feature: &InlineFeature) -> Geometry {
    let coords = |part: &Vec<[f64; 2]>| -> Vec<Coord> {
        part.iter().map(|&[x, y]| Coord { x, y }).collect()
    };
    let parts = &feature.parts;
    match feature.shape_type {
        ShapeType::Point => match parts.as_slice() {
            [part] if part.len() == 1 => Geometry::Point(coords(part)[0]),
            _ => Geometry::MultiPoint(parts.iter().flat_map(coords).collect()),
        },
        ShapeType::Line => match parts.as_slice() {
            [part] => Geometry::LineString(coords(part)),
            _ => Geometry::MultiLineString(parts.iter().map(coords).collect()),
        },
        ShapeType::Polygon => Geometry::Polygon(parts.iter().map(coords).collect()),
    }
}

/// The values of each feature, by name.
#[derive(Debug)]
struct Values(Vec<Vec<(String, String)>>);

impl Table for Values {
    /// The value of the first property whose name matches `name` without regard to ASCII case,
    /// as `msLayerGetItemIndex` compares them.
    fn get(&self, record: u64, name: &str) -> Option<Value<'_>> {
        let properties = self.0.get(usize::try_from(record).ok()?)?;
        properties
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| Value::String(Cow::Borrowed(v)))
    }

    fn each(&self, record: u64, f: &mut dyn FnMut(&str, Value<'_>)) {
        let Some(properties) = usize::try_from(record).ok().and_then(|r| self.0.get(r)) else {
            return;
        };
        for (n, v) in properties {
            f(n, Value::String(Cow::Borrowed(v)));
        }
    }
}

#[cfg(test)]
mod tests {
    use forager_ports::poll_once;
    use forager_style_ir::FeatureProperty;

    use super::*;

    fn feature(shape_type: ShapeType, parts: &[&[[f64; 2]]]) -> InlineFeature {
        InlineFeature {
            shape_type,
            parts: parts.iter().map(|p| p.to_vec()).collect(),
            properties: Vec::new(),
            text: None,
            span: None,
        }
    }

    fn query(source: &InlineSource, extent: [f64; 4]) -> Vec<Feature> {
        let [min_x, min_y, max_x, max_y] = extent;
        let query = FeatureQuery {
            extent: Extent {
                min_x,
                min_y,
                max_x,
                max_y,
            },
        };
        poll_once(source.features(&query))
            .unwrap()
            .unwrap()
            .features
    }

    #[test]
    fn features_meeting_the_extent_are_delivered_in_order_with_their_place_as_id() {
        let features = [
            feature(ShapeType::Point, &[&[[5.0, 5.0]]]),
            feature(ShapeType::Point, &[&[[50.0, 50.0]]]),
            // Its bounds touch the extent's edge.
            feature(ShapeType::Line, &[&[[10.0, 0.0], [20.0, 0.0]]]),
            // Without points, its bounds are msInitShape's -1.
            feature(ShapeType::Polygon, &[]),
        ];
        let source = InlineSource::new(&features);
        let ids = |extent| -> Vec<u64> { query(&source, extent).iter().map(|f| f.id).collect() };
        assert_eq!(ids([0.0, 0.0, 10.0, 10.0]), [0, 2]);
        assert_eq!(ids([-2.0, -2.0, 0.0, 0.0]), [3]);
    }

    #[test]
    fn a_shape_is_delivered_as_the_shapefile_source_delivers_a_record() {
        let geometry_of = |f: InlineFeature| geometry(&f);
        let c = |x, y| Coord { x, y };
        assert_eq!(
            geometry_of(feature(ShapeType::Point, &[&[[1.0, 2.0]]])),
            Geometry::Point(c(1.0, 2.0))
        );
        assert_eq!(
            geometry_of(feature(ShapeType::Point, &[&[[1.0, 2.0]], &[[3.0, 4.0]]])),
            Geometry::MultiPoint(vec![c(1.0, 2.0), c(3.0, 4.0)])
        );
        assert_eq!(
            geometry_of(feature(ShapeType::Line, &[&[[1.0, 2.0], [3.0, 4.0]]])),
            Geometry::LineString(vec![c(1.0, 2.0), c(3.0, 4.0)])
        );
        assert_eq!(
            geometry_of(feature(ShapeType::Line, &[&[[1.0, 2.0]], &[[3.0, 4.0]]])),
            Geometry::MultiLineString(vec![vec![c(1.0, 2.0)], vec![c(3.0, 4.0)]])
        );
        assert_eq!(
            geometry_of(feature(ShapeType::Polygon, &[&[[1.0, 2.0]], &[[3.0, 4.0]]])),
            Geometry::Polygon(vec![vec![c(1.0, 2.0)], vec![c(3.0, 4.0)]])
        );
    }

    #[test]
    fn a_property_is_found_without_regard_to_case_the_first_winning() {
        let mut f = feature(ShapeType::Point, &[&[[0.0, 0.0]]]);
        f.properties = [("Name", "a"), ("NAME", "b"), ("size", "")]
            .map(|(name, value)| FeatureProperty {
                name: name.to_owned(),
                value: value.to_owned(),
            })
            .to_vec();
        let features = [f];
        let source = InlineSource::new(&features);
        let delivered = query(&source, [-1.0, -1.0, 1.0, 1.0]);
        let properties = &delivered[0].properties;
        assert_eq!(properties.get("name"), Some(Value::String("a".into())));
        assert_eq!(properties.get("SIZE"), Some(Value::String("".into())));
        assert_eq!(properties.get("other"), None);
        let mut names = Vec::new();
        properties.each(|n, _| names.push(n.to_owned()));
        assert_eq!(names, ["Name", "NAME", "size"]);
    }
}
