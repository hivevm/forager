use std::path::{Path, PathBuf};

use forager_ports::{ResourceLoader, ResourcePath, SourceError};

/// Reads resources from a directory, the base context. A [`ResourcePath`] cannot climb out of
/// it; the canonical path is checked as well, so a symbolic link cannot lead out either (Q-8).
pub struct FileLoader {
    root: PathBuf,
    /// The largest file a data source may read, refused past it (Q-9); `None` for the files of a
    /// mapfile, whose limit the frontend applies to all of them together.
    data_limit: Option<u64>,
}

impl FileLoader {
    /// The loader for `base`, by default the mapfile's directory, and the mapfile's path inside
    /// it. The mapfile must lie inside `base`.
    pub fn for_mapfile(mapfile: &Path, base: Option<&Path>) -> Result<(Self, String), String> {
        let mapfile = mapfile.canonicalize().map_err(|e| e.to_string())?;
        let root = match base {
            Some(base) => base
                .canonicalize()
                .map_err(|e| format!("--base {}: {e}", base.display()))?,
            None => mapfile
                .parent()
                .ok_or("the mapfile has no directory")?
                .to_path_buf(),
        };
        let inside = mapfile.strip_prefix(&root).map_err(|_| {
            format!(
                "the mapfile lies outside the base context {}",
                root.display()
            )
        })?;
        let name = inside
            .iter()
            .map(|part| part.to_str())
            .collect::<Option<Vec<_>>>()
            .ok_or("the mapfile's path is not valid UTF-8")?
            .join("/");
        Ok((
            Self {
                root,
                data_limit: None,
            },
            name,
        ))
    }

    /// Whether `path` names a regular file inside the base context.
    pub fn holds(&self, path: &ResourcePath) -> bool {
        self.root
            .join(path.as_str())
            .canonicalize()
            .is_ok_and(|full| full.starts_with(&self.root) && full.is_file())
    }

    /// The loader of the same base context for the files of a data source, each refused past
    /// `limit` bytes (Q-9).
    pub fn for_data(self, limit: u64) -> Self {
        Self {
            data_limit: Some(limit),
            ..self
        }
    }
}

impl ResourceLoader for FileLoader {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        let full = self
            .root
            .join(path.as_str())
            .canonicalize()
            .map_err(|e| SourceError::new(e.to_string()))?;
        if !full.starts_with(&self.root) {
            return Err(SourceError::new("the path leaves the base context"));
        }
        let limit = self
            .data_limit
            .unwrap_or(forager_mapfile::MAX_TOTAL_BYTES as u64);
        let bytes = read_bounded(&full, limit).map_err(|e| SourceError::new(e.to_string()))?;
        if self
            .data_limit
            .is_some_and(|limit| bytes.len() as u64 > limit)
        {
            return Err(SourceError::new(format!(
                "the file exceeds the limit of {limit} bytes"
            )));
        }
        Ok(bytes)
    }
}

/// The bytes of a regular file, at most one past the limit of a load, which then reports it
/// (Q-9). A pipe or a device is refused unopened, since opening or reading one may wait or never
/// end; the file opened is checked again.
fn read_bounded(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let not_regular = || std::io::Error::other("not a regular file");
    if !std::fs::metadata(path)?.is_file() {
        return Err(not_regular());
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(not_regular());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_file_past_the_limit_is_refused() {
        // Verifies: Q-9
        let dir = std::env::temp_dir().join(format!("forager-data-limit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.map"), "MAP END").unwrap();
        std::fs::write(dir.join("at.shp"), [0u8; 10]).unwrap();
        std::fs::write(dir.join("past.shp"), [0u8; 11]).unwrap();
        let (loader, _) = FileLoader::for_mapfile(&dir.join("m.map"), None).unwrap();
        let loader = loader.for_data(10);
        let load = |name: &str| {
            forager_ports::poll_once(loader.load(&ResourcePath::new(name).unwrap())).unwrap()
        };
        assert_eq!(load("at.shp").unwrap().len(), 10);
        let error = load("past.shp").unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            error.message.contains("exceeds the limit of 10 bytes"),
            "{error}"
        );
    }
}
