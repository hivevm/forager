//! The GeoJSON feature source (ADR-0016): an RFC 7946 document, parsed once, delivered as the
//! features that touch a query's extent.

use forager_ports::{
    Coord, Feature, FeatureQuery, FeatureSource, Geometry, Properties, SourceError, Stored,
};
use forager_style_ir::Extent;
use geojson::{GeoJson, GeometryValue, JsonObject, JsonValue, Position};

/// The features of one GeoJSON document, numbered in document order.
pub struct GeoJsonSource {
    features: Vec<(Feature, Extent)>,
}

impl GeoJsonSource {
    /// Parses a GeoJSON document: a feature collection, a feature, or a bare geometry. Features
    /// without a geometry are left out.
    pub fn parse(text: &str) -> Result<Self, SourceError> {
        let document: GeoJson = text
            .parse()
            .map_err(|e: geojson::Error| SourceError::new(format!("invalid GeoJSON: {e}")))?;
        // A feature's id is its index in the document, features without a geometry counted.
        let parts: Vec<(Option<geojson::Geometry>, Option<JsonObject>)> = match document {
            GeoJson::FeatureCollection(collection) => collection
                .features
                .into_iter()
                .map(|f| (f.geometry, f.properties))
                .collect(),
            GeoJson::Feature(feature) => vec![(feature.geometry, feature.properties)],
            GeoJson::Geometry(geometry) => vec![(Some(geometry), None)],
        };
        let mut features = Vec::with_capacity(parts.len());
        for (id, (geometry, object)) in (0u64..).zip(parts) {
            let Some(geometry) = geometry else { continue };
            let geometry = convert(&geometry.value)?;
            if let Some(bounds) = bounds(&geometry) {
                let properties = object.map_or_else(Properties::default, properties);
                features.push((
                    Feature {
                        id,
                        geometry,
                        properties,
                    },
                    bounds,
                ));
            }
        }
        Ok(Self { features })
    }
}

impl FeatureSource for GeoJsonSource {
    async fn features(&self, query: &FeatureQuery) -> Result<Vec<Feature>, SourceError> {
        let q = &query.extent;
        Ok(self
            .features
            .iter()
            .filter(|(_, b)| {
                b.min_x <= q.max_x && b.max_x >= q.min_x && b.min_y <= q.max_y && b.max_y >= q.min_y
            })
            .map(|(feature, _)| feature.clone())
            .collect())
    }
}

/// A feature's `properties` as values of the port (ADR-0026), sorted by name as the parser keeps
/// them, of two of one name the last: a number as the text `serde_json` prints for it, an array or
/// object as its JSON text.
fn properties(object: JsonObject) -> Properties {
    let list = object
        .into_iter()
        .map(|(name, value)| {
            let value = match value {
                JsonValue::Null => Stored::Null,
                JsonValue::Bool(b) => Stored::Bool(b),
                JsonValue::Number(n) => Stored::Number(n.to_string().into()),
                JsonValue::String(text) => Stored::String(text.into()),
                nested => Stored::String(nested.to_string().into()),
            };
            (name.into(), value)
        })
        .collect();
    Properties::list(list)
}

fn coord(position: &Position) -> Result<Coord, SourceError> {
    match position.as_slice() {
        [x, y, ..] => Ok(Coord { x: *x, y: *y }),
        _ => Err(SourceError::new("a position has fewer than two numbers")),
    }
}

fn coords(positions: &[Position]) -> Result<Vec<Coord>, SourceError> {
    positions.iter().map(coord).collect()
}

fn rings(rings: &[Vec<Position>]) -> Result<Vec<Vec<Coord>>, SourceError> {
    rings.iter().map(|r| coords(r)).collect()
}

fn convert(value: &GeometryValue) -> Result<Geometry, SourceError> {
    Ok(match value {
        GeometryValue::Point { coordinates } => Geometry::Point(coord(coordinates)?),
        GeometryValue::MultiPoint { coordinates } => Geometry::MultiPoint(coords(coordinates)?),
        GeometryValue::LineString { coordinates } => Geometry::LineString(coords(coordinates)?),
        GeometryValue::MultiLineString { coordinates } => {
            Geometry::MultiLineString(rings(coordinates)?)
        }
        GeometryValue::Polygon { coordinates } => Geometry::Polygon(rings(coordinates)?),
        GeometryValue::MultiPolygon { coordinates } => Geometry::MultiPolygon(
            coordinates
                .iter()
                .map(|p| rings(p))
                .collect::<Result<_, _>>()?,
        ),
        GeometryValue::GeometryCollection { geometries } => Geometry::Collection(
            geometries
                .iter()
                .map(|g| convert(&g.value))
                .collect::<Result<_, _>>()?,
        ),
    })
}

/// The bounding box of a geometry's finite coordinates, or `None` if it has none.
fn bounds(geometry: &Geometry) -> Option<Extent> {
    let mut extent: Option<Extent> = None;
    visit(geometry, &mut |c| {
        if !(c.x.is_finite() && c.y.is_finite()) {
            return;
        }
        let e = extent.get_or_insert(Extent {
            min_x: c.x,
            min_y: c.y,
            max_x: c.x,
            max_y: c.y,
        });
        e.min_x = e.min_x.min(c.x);
        e.min_y = e.min_y.min(c.y);
        e.max_x = e.max_x.max(c.x);
        e.max_y = e.max_y.max(c.y);
    });
    extent
}

fn visit(geometry: &Geometry, f: &mut impl FnMut(Coord)) {
    let mut each = |cs: &[Coord]| cs.iter().copied().for_each(&mut *f);
    match geometry {
        Geometry::Point(c) => each(std::slice::from_ref(c)),
        Geometry::MultiPoint(cs) | Geometry::LineString(cs) => each(cs),
        Geometry::MultiLineString(lines) | Geometry::Polygon(lines) => {
            lines.iter().for_each(|l| each(l));
        }
        Geometry::MultiPolygon(polygons) => polygons.iter().flatten().for_each(|r| each(r)),
        Geometry::Collection(geometries) => geometries.iter().for_each(|g| visit(g, f)),
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use forager_ports::Value;

    use super::*;

    fn ready<F: Future>(future: F) -> F::Output {
        forager_ports::poll_once(future).expect("the GeoJSON source never waits")
    }

    fn query(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> FeatureQuery {
        FeatureQuery {
            extent: Extent {
                min_x,
                min_y,
                max_x,
                max_y,
            },
        }
    }

    const ALL_TYPES: &str = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {}, "geometry": {"type": "Point", "coordinates": [1, 2]}},
        {"type": "Feature", "properties": {}, "geometry": null},
        {"type": "Feature", "properties": {}, "geometry": {"type": "MultiPoint", "coordinates": [[1, 2], [3, 4]]}},
        {"type": "Feature", "properties": {}, "geometry": {"type": "LineString", "coordinates": [[0, 0], [5, 5]]}},
        {"type": "Feature", "properties": {}, "geometry": {"type": "MultiLineString", "coordinates": [[[0, 0], [1, 1]]]}},
        {"type": "Feature", "properties": {}, "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [4, 0], [4, 4], [0, 0]]]}},
        {"type": "Feature", "properties": {}, "geometry": {"type": "MultiPolygon", "coordinates": [[[[10, 10], [12, 10], [12, 12], [10, 10]]]]}},
        {"type": "Feature", "properties": {}, "geometry": {"type": "GeometryCollection", "geometries": [{"type": "Point", "coordinates": [7, 7]}]}},
        {"type": "Feature", "properties": {}, "geometry": null}
    ]}"#;

    #[test]
    fn every_geometry_type_is_read_in_document_order() {
        // Verifies: ADR-0016
        let source = GeoJsonSource::parse(ALL_TYPES).unwrap();
        let features = ready(source.features(&query(-100.0, -100.0, 100.0, 100.0))).unwrap();
        assert_eq!(features.len(), 7);
        // The feature without a geometry keeps its index: ids skip 1 and 8.
        assert_eq!(
            features.iter().map(|f| f.id).collect::<Vec<_>>(),
            [0, 2, 3, 4, 5, 6, 7]
        );
        assert_eq!(
            features[0].geometry,
            Geometry::Point(Coord { x: 1.0, y: 2.0 })
        );
        assert!(matches!(&features[4].geometry, Geometry::Polygon(rings) if rings[0].len() == 4));
        assert!(matches!(&features[6].geometry, Geometry::Collection(g) if g.len() == 1));
    }

    #[test]
    fn a_feature_carries_each_property_of_its_kind() {
        // Verifies: ADR-0026
        let source = GeoJsonSource::parse(
            r#"{"type": "FeatureCollection", "features": [
                {"type": "Feature", "geometry": {"type": "Point", "coordinates": [1, 2]},
                 "properties": {"name": "Wien", "pop": 1.50, "big": 12345678901234567890,
                                "capital": true, "note": null, "tags": [1, "a"],
                                "Name": "other"}},
                {"type": "Feature", "geometry": {"type": "Point", "coordinates": [1, 2]},
                 "properties": null}
            ]}"#,
        )
        .unwrap();
        let features = ready(source.features(&query(0.0, 0.0, 5.0, 5.0))).unwrap();
        let p = &features[0].properties;
        assert_eq!(p.get("name"), Some(Value::String("Wien".into())));
        assert_eq!(p.get("Name"), Some(Value::String("other".into())));
        assert_eq!(p.get("pop"), Some(Value::Number("1.5")));
        assert_eq!(p.get("big"), Some(Value::Number("12345678901234567890")));
        assert_eq!(p.get("capital"), Some(Value::Bool(true)));
        assert_eq!(p.get("note"), Some(Value::Null));
        assert_eq!(p.get("tags"), Some(Value::String(r#"[1,"a"]"#.into())));
        assert_eq!(p.get("NAME"), None);
        assert_eq!(features[1].properties.get("name"), None);
    }

    #[test]
    fn only_features_touching_the_extent_are_delivered() {
        let source = GeoJsonSource::parse(ALL_TYPES).unwrap();
        let features = ready(source.features(&query(9.0, 9.0, 11.0, 11.0))).unwrap();
        assert_eq!(features.iter().map(|f| f.id).collect::<Vec<_>>(), [6]);
    }

    #[test]
    fn malformed_documents_are_errors_not_panics() {
        // Verifies: ADR-0016, Q-3
        for text in [
            "",
            "{",
            r#"{"type": "Point"}"#,
            r#"{"type": "Point", "coordinates": [1]}"#,
            r#"{"type": "Banana", "coordinates": [1, 2]}"#,
        ] {
            assert!(GeoJsonSource::parse(text).is_err(), "{text}");
        }
    }
}
