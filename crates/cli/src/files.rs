use std::path::{Path, PathBuf};

use forager_ports::{ResourceLoader, ResourcePath, SourceError};

/// Reads resources from a directory, the base context. A [`ResourcePath`] cannot climb out of
/// it; the canonical path is checked as well, so a symbolic link cannot lead out either (Q-8).
/// A file it cannot read, missing or outside, is one and the same error, so that a link in the
/// base context cannot tell whether a path outside it exists.
#[derive(Clone)]
pub struct FileLoader {
    root: PathBuf,
}

/// Reads the files of a data source from the base context, each refused past its limit (Q-9).
/// A type of its own, so that a data source cannot be handed the loader of a mapfile, whose
/// limit the frontend applies to all its files together.
pub struct DataLoader {
    files: FileLoader,
    limit: u64,
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
        Ok((Self { root }, name))
    }

    /// The loader of the same base context for the files of a data source, each refused past
    /// `limit` bytes (Q-9).
    pub fn for_data(self, limit: u64) -> DataLoader {
        DataLoader { files: self, limit }
    }

    /// The canonical path of `path` where it is a file inside the base context; `None` for one
    /// that is missing or that a link leads out of, alike.
    fn inside(&self, path: &ResourcePath) -> Option<PathBuf> {
        let full = self.root.join(path.as_str()).canonicalize().ok()?;
        full.starts_with(&self.root).then_some(full)
    }

    fn read(&self, path: &ResourcePath, limit: u64) -> Result<Vec<u8>, SourceError> {
        let full = self
            .inside(path)
            .ok_or_else(|| SourceError::new("no such file, or the path leaves the base context"))?;
        read_bounded(&full, limit).map_err(|e| SourceError::new(e.to_string()))
    }
}

impl ResourceLoader for FileLoader {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.read(path, forager_mapfile::MAX_TOTAL_BYTES as u64)
    }
}

impl DataLoader {
    /// Whether `path` names a regular file inside the base context.
    pub fn holds(&self, path: &ResourcePath) -> bool {
        self.files.inside(path).is_some_and(|full| full.is_file())
    }
}

impl ResourceLoader for DataLoader {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        let bytes = self.files.read(path, self.limit)?;
        if bytes.len() as u64 > self.limit {
            return Err(SourceError::new(format!(
                "the file exceeds the limit of {} bytes",
                self.limit
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
