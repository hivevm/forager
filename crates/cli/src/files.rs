use std::future::Future;
use std::path::{Path, PathBuf};
use std::task::{Context, Poll, Waker};

use forager_ports::{ResourceLoader, ResourcePath, SourceError};

/// Reads resources from a directory, the base context. A [`ResourcePath`] cannot climb out of
/// it; the canonical path is checked as well, so a symbolic link cannot lead out either (Q-8).
pub struct FileLoader {
    root: PathBuf,
}

impl FileLoader {
    /// The loader for a mapfile's directory, and the mapfile's name inside it.
    pub fn for_mapfile(mapfile: &Path) -> Result<(Self, String), String> {
        let mapfile = mapfile.canonicalize().map_err(|e| e.to_string())?;
        let root = mapfile.parent().ok_or("the mapfile has no directory")?;
        let name = mapfile
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("the mapfile's name is not valid UTF-8")?;
        Ok((
            Self {
                root: root.to_path_buf(),
            },
            name.to_owned(),
        ))
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
        std::fs::read(&full).map_err(|e| SourceError::new(e.to_string()))
    }
}

/// Runs a future to completion on this thread. The file loader never waits, so the future is
/// ready on its first poll; the loop only guards against a future that is not.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}
