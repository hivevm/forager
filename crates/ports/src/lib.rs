//! The ports: the traits at the edge of the core through which a host supplies data and
//! resources (ADR-0009). They are the only place async appears, and none of them requires
//! `Send`, because the browser is single-threaded.

use std::fmt;

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
