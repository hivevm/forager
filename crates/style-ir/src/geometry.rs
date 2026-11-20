//! The geometry of a feature, as a source delivers it through the ports and the symbolization draws
//! it. It lives in the IR, the one crate both may depend on (ARCH-crates-as-ports-and-adapters);
//! the ports re-export it.

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
    /// A point shape's parts, each a list of points, as MapServer's shape holds them. A feature
    /// source delivers one part, but an inline feature one per `POINTS` block.
    MultiPoint(Vec<Vec<Coord>>),
    LineString(Vec<Coord>),
    MultiLineString(Vec<Vec<Coord>>),
    Polygon(Vec<Vec<Coord>>),
    MultiPolygon(Vec<Vec<Vec<Coord>>>),
    Collection(Vec<Geometry>),
}
