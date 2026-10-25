//! The ports: the traits at the edge of the core through which a host supplies data and
//! resources (ADR-0009), and `poll_once`, which runs a future over them that never waits. They
//! are the only place async appears, and none of them requires `Send`, because the browser is
//! single-threaded.

use std::fmt;

use forager_style_ir::Extent;

/// A path inside the base context: relative, normalized, and unable to leave it (Q-8).
///
/// Built only by [`ResourcePath::new`] and [`ResourcePath::resolve`], which refuse absolute paths
/// and any `..` that would climb out of the base context, so a loader that joins it to its root
/// cannot escape.
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
        match parts.first() {
            None => Err(PathError::Empty),
            // A `..` can drop what preceded a drive prefix, so the normalized path is checked too.
            Some(first) if has_drive_prefix(first) => Err(PathError::Absolute),
            Some(_) => Ok(Self(parts.join("/"))),
        }
    }

    /// Normalizes `raw`, a path as written in a style, relative to the directory of this path, as
    /// MapServer resolves an `INCLUDE` against the main mapfile's directory. A `..` may climb out
    /// of that directory but not out of the base context.
    pub fn resolve(&self, raw: &str) -> Result<Self, PathError> {
        if raw.starts_with(['/', '\\']) || has_drive_prefix(raw) {
            return Err(PathError::Absolute);
        }
        match self.0.rsplit_once('/') {
            Some((dir, _)) => Self::new(&format!("{dir}/{raw}")),
            None => Self::new(raw),
        }
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

/// Runs a future whose ports never wait — every port reads from memory or a local file — in one
/// poll, and returns `None` for a future that waits. Nothing could wake it: a host without an
/// executor would spin on it, as a browser tab would, or hang. Every host and test that runs the
/// frontend or the engine synchronously runs it here.
pub fn poll_once<F: Future>(future: F) -> Option<F::Output> {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
    {
        std::task::Poll::Ready(output) => Some(output),
        std::task::Poll::Pending => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_future_ready_on_its_first_poll_runs_to_its_output() {
        assert_eq!(poll_once(async { 7 }), Some(7));
    }

    /// Waits once, counting its polls, and is ready on the next.
    struct WaitsOnce<'a>(&'a std::cell::Cell<u32>);

    impl Future for WaitsOnce<'_> {
        type Output = ();
        fn poll(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<()> {
            self.0.set(self.0.get() + 1);
            if self.0.get() == 1 {
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        }
    }

    #[test]
    fn a_future_that_waits_is_none_and_polled_once() {
        let polls = std::cell::Cell::new(0);
        assert_eq!(poll_once(WaitsOnce(&polls)), None);
        assert_eq!(polls.get(), 1);
    }

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
    fn a_path_resolves_against_the_directory_of_another_inside_the_base_context() {
        // Verifies: Q-8
        let main = ResourcePath::new("maps/world.map").unwrap();
        assert_eq!(
            main.resolve("inc/a.inc").unwrap().as_str(),
            "maps/inc/a.inc"
        );
        assert_eq!(main.resolve("../inc/a.inc").unwrap().as_str(), "inc/a.inc");
        assert_eq!(
            main.resolve("../../secret"),
            Err(PathError::EscapesBaseContext)
        );
        assert_eq!(main.resolve("/etc/passwd"), Err(PathError::Absolute));
        assert_eq!(main.resolve("C:\\Windows"), Err(PathError::Absolute));
        let top = ResourcePath::new("world.map").unwrap();
        assert_eq!(top.resolve("a.inc").unwrap().as_str(), "a.inc");
        assert_eq!(top.resolve("../a.inc"), Err(PathError::EscapesBaseContext));
    }

    #[test]
    fn a_drive_prefix_that_normalization_uncovers_is_refused() {
        // Verifies: Q-8
        assert_eq!(
            ResourcePath::new("a/../C:/Windows"),
            Err(PathError::Absolute)
        );
        assert_eq!(ResourcePath::new("./c:relative"), Err(PathError::Absolute));
        assert_eq!(ResourcePath::new("/../C:x"), Err(PathError::Absolute));
        let main = ResourcePath::new("maps/world.map").unwrap();
        assert_eq!(main.resolve("x/../../C:x"), Err(PathError::Absolute));
        assert_eq!(main.resolve("../d:\\secret"), Err(PathError::Absolute));
        // A colon past the first component names no drive: Windows reads a prefix only first.
        assert_eq!(ResourcePath::new("a/C:/b").unwrap().as_str(), "a/C:/b");
    }

    #[test]
    fn every_spelling_of_an_escape_is_refused() {
        // Verifies: Q-8
        for raw in [
            "..",
            "../",
            "..\\secret",
            "a\\..\\..\\secret",
            "./../secret",
            "a//..//..//secret",
            "a/./../../secret",
            "a/b/../../../secret",
        ] {
            assert_eq!(
                ResourcePath::new(raw),
                Err(PathError::EscapesBaseContext),
                "{raw}"
            );
        }
        for raw in ["\\\\server\\share\\x", "//server/share/x", "\\x", "/"] {
            assert_eq!(ResourcePath::new(raw), Err(PathError::Absolute), "{raw}");
        }
        let main = ResourcePath::new("maps/world.map").unwrap();
        assert_eq!(main.resolve("\\\\server\\x"), Err(PathError::Absolute));
        assert_eq!(main.resolve("a\0b"), Err(PathError::InvalidCharacter));
        assert_eq!(
            main.resolve("..\\..\\secret"),
            Err(PathError::EscapesBaseContext)
        );
    }

    #[test]
    fn what_is_no_separator_or_dot_segment_stays_a_name() {
        // Verifies: Q-8
        // Percent-encoding is a URL's, not a path's: `%2e%2e` names a file, and a loader that
        // turns the path into a URL has to encode it so (the URL parser reads `%2e%2e` as `..`).
        assert_eq!(
            ResourcePath::new("%2e%2e/secret").unwrap().as_str(),
            "%2e%2e/secret"
        );
        assert_eq!(ResourcePath::new(".../x").unwrap().as_str(), ".../x");
        assert_eq!(ResourcePath::new("..a/x").unwrap().as_str(), "..a/x");
        assert_eq!(ResourcePath::new("inc/").unwrap().as_str(), "inc");
        assert_eq!(ResourcePath::new("a/./b/.//c/").unwrap().as_str(), "a/b/c");
        let long = "a/".repeat(5000) + "x";
        assert_eq!(ResourcePath::new(&long).unwrap().as_str(), long);
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
/// proj4 string (ADR-0023). Synchronous: a host that fetches its definitions loads them before it
/// loads the style.
pub trait ProjectionResolver {
    /// The proj4 string for `code` of `authority` (such as `epsg`, case-insensitive), or `None`
    /// when the resolver does not know it.
    fn resolve(&self, authority: &str, code: &str) -> Option<String>;
}
