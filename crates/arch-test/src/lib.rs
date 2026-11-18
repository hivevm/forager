//! No code: this crate holds the dependency test of ADR-0008 under `tests/`.

use std::path::{Path, PathBuf};

/// This crate's directory: the run time's `CARGO_MANIFEST_DIR`, else the compiled-in path.
#[must_use]
pub fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

/// Every `.rs` file under `dir`, `target` directories left out, in the order the directories list
/// them; a directory that cannot be read, `dir` itself included, adds none.
///
/// # Panics
///
/// Where an entry of a directory that was read cannot be read.
#[must_use]
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk(dir, &mut files);
    files
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.map(Result::unwrap) {
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() != "target" {
                walk(&path, files);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}
