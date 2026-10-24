//! The ports: the traits at the edge of the core through which a host supplies data and
//! resources (ADR-0009). They are the only place async appears, and none of them requires
//! `Send`, because the browser is single-threaded.

use std::fmt;

use forager_style_ir::Extent;

/// A path inside the base context: relative, normalized, and unable to leave it (Q-8).
///
/// Built only by [`ResourcePath::new`], which refuses absolute paths and any `..` that would
/// climb out of the base context, so a loader that joins it to its root cannot escape.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourcePath(String);

/// Why a path was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Absolute,
    EscapesBaseContext,
    InvalidCharacter,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PathError::Empty => "the path is empty",
            PathError::Absolute => "absolute paths are not allowed",
            PathError::EscapesBaseContext => "the path leaves the base context",
            PathError::InvalidCharacter => "the path contains a NUL character",
        })
    }
}

impl ResourcePath {
    /// Normalizes `raw`, a path as written in a style, relative to the base context. Both `/` and
    /// `\` separate components.
    pub fn new(raw: &str) -> Result<Self, PathError> {
        if raw.contains('\0') {
            return Err(PathError::InvalidCharacter);
        }
        if raw.starts_with(['/', '\\']) || has_drive_prefix(raw) {
            return Err(PathError::Absolute);
        }
        let mut parts: Vec<&str> = Vec::new();
        for part in raw.split(['/', '\\']) {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop().ok_or(PathError::EscapesBaseContext)?;
                }
                _ => parts.push(part),
            }
        }
        if parts.is_empty() {
            return Err(PathError::Empty);
        }
        Ok(Self(parts.join("/")))
    }

    /// The normalized path, components separated by `/`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn has_drive_prefix(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

impl fmt::Display for ResourcePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A failure of a port, reported by its adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceError {
    pub message: String,
}

impl SourceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Supplies the bytes of the files a style refers to: `INCLUDE`s, symbol sets, font sets, fonts,
/// and pixmaps.
// `async fn` in a public trait cannot state `Send` for its future; that is the point here, since
// the browser's futures are not `Send` (specification, Constraints).
#[allow(async_fn_in_trait)]
pub trait ResourceLoader {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_are_normalized() {
        assert_eq!(
            ResourcePath::new("a/./b//c.map").unwrap().as_str(),
            "a/b/c.map"
        );
        assert_eq!(
            ResourcePath::new("a\\b\\..\\c.map").unwrap().as_str(),
            "a/c.map"
        );
        assert_eq!(ResourcePath::new("a/../c.map").unwrap().as_str(), "c.map");
    }

    #[test]
    fn paths_that_leave_the_base_context_are_refused() {
        // Verifies: Q-8
        assert_eq!(
            ResourcePath::new("../secret"),
            Err(PathError::EscapesBaseContext)
        );
        assert_eq!(
            ResourcePath::new("a/../../secret"),
            Err(PathError::EscapesBaseContext)
        );
        assert_eq!(ResourcePath::new("/etc/passwd"), Err(PathError::Absolute));
        assert_eq!(
            ResourcePath::new("\\\\server\\share"),
            Err(PathError::Absolute)
        );
        assert_eq!(ResourcePath::new("C:\\Windows"), Err(PathError::Absolute));
        assert_eq!(ResourcePath::new("c:relative"), Err(PathError::Absolute));
        assert_eq!(ResourcePath::new("a\0b"), Err(PathError::InvalidCharacter));
    }

    #[test]
    fn a_path_that_names_nothing_is_refused() {
        assert_eq!(ResourcePath::new(""), Err(PathError::Empty));
        assert_eq!(ResourcePath::new("./"), Err(PathError::Empty));
        assert_eq!(ResourcePath::new("a/.."), Err(PathError::Empty));
    }
}

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

/// A feature as a source delivers it. Its id is stable for a source, so that output that depends
/// on feature order stays deterministic (Q-2).
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub id: u64,
    pub geometry: Geometry,
}

/// What the core asks a feature source for.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureQuery {
    /// The area of interest, in the source's coordinates.
    pub extent: Extent,
}

/// Delivers the features of one layer's source.
// Not `Send`, for the reason given at `ResourceLoader`.
#[allow(async_fn_in_trait)]
pub trait FeatureSource {
    async fn features(&self, query: &FeatureQuery) -> Result<Vec<Feature>, SourceError>;
}

/// Resolves a projection a style names by authority and code, as `init=epsg:4326` does, to a
/// proj4 string (ADR-0018). Synchronous: a host that fetches its definitions loads them before it
/// loads the style.
pub trait ProjectionResolver {
    /// The proj4 string for `code` of `authority` (such as `epsg`, case-insensitive), or `None`
    /// when the resolver does not know it.
    fn resolve(&self, authority: &str, code: &str) -> Option<String>;
}
