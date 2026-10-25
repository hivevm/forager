use std::path::{Path, PathBuf};

use forager_ports::{ResourceLoader, ResourcePath, SourceError};

/// Reads resources from a directory, the base context. A [`ResourcePath`] cannot climb out of
/// it; the canonical path is checked as well, so a symbolic link cannot lead out either (Q-8).
pub struct FileLoader {
    root: PathBuf,
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
        read_bounded(&full).map_err(|e| SourceError::new(e.to_string()))
    }
}

/// The bytes of a regular file, at most one past the limit of a load, which then reports it
/// (Q-9). A pipe or a device is refused unopened, since opening or reading one may wait or never
/// end; the file opened is checked again.
fn read_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
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
    file.take(forager_mapfile::MAX_TOTAL_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}
