//! The architecture checks of the workspace, each a test binary under `tests/`: the dependency
//! edges between the crates and what each may link or lift (`dependencies.rs`, ADR-0008), the
//! direction of the imports between the modules of the engine and of `symbolize`
//! (`engine_modules.rs` and `symbolize_modules.rs`, both reading them by `module_direction/`),
//! C's and MapServer's small ports defined once (`c_numbers.rs`, ADR-0017), a symbol's name
//! compared once (`symbol_names.rs`), and the crate's directory taken at run time
//! (`manifest_dir.rs`). This lib holds what the checks that read the tree share, the crate's
//! directory and the walk of the `.rs` files under a directory; no other crate depends on it.

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
