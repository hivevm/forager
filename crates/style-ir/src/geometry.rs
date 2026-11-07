//! The geometry of a feature, as a source delivers it through the ports and the symbolization
//! draws it. It lives in the IR, the one crate both may depend on (ADR-0009); the ports re-export
//! it.

/// A position in the coordinates of its source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
}

/// The geometry of a feature. A polygon is its rings, the exterior first.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point(Coord),
    MultiPoint(Vec<Coord>),
    LineString(Vec<Coord>),
    MultiLineString(Vec<Vec<Coord>>),
    Polygon(Vec<Vec<Coord>>),
    MultiPolygon(Vec<Vec<Vec<Coord>>>),
    Collection(Vec<Geometry>),
}
