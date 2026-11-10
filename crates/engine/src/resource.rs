//! The one load of the files a render's fonts and pixmap symbols' images are read from: through
//! the host's resource loader once per render and path, each file held to Q-9's bound
//! (ADR-0029, ADR-0031).

use std::collections::BTreeMap;

use forager_ports::{ResourceLoader, ResourcePath};

/// The largest file of a font or of a pixmap symbol's image a render loads (Q-9).
pub const MAX_FONT_OR_IMAGE_FILE_BYTES: usize = 32 * 1024 * 1024;

/// What `read` makes of the file at `path`, kept in `loaded` by its path: the file is loaded
/// through `loader` and held to [`MAX_FONT_OR_IMAGE_FILE_BYTES`] the first time `path` is asked
/// for, and every later call takes what was kept. `read` is handed the file, or why there is
/// none, and its result is kept, its reason included.
pub(crate) async fn load_once<L: ResourceLoader, T: Clone>(
    loaded: &mut BTreeMap<String, Result<T, String>>,
    path: &str,
    loader: &L,
    read: impl FnOnce(Result<Vec<u8>, String>) -> Result<T, String>,
) -> Result<T, String> {
    if let Some(kept) = loaded.get(path) {
        return kept.clone();
    }
    let kept = read(bounded(path, loader).await);
    loaded.insert(path.to_owned(), kept.clone());
    kept
}

/// The file at `path`, or why it does not load.
async fn bounded<L: ResourceLoader>(path: &str, loader: &L) -> Result<Vec<u8>, String> {
    let resource =
        ResourcePath::new(path).map_err(|e| format!("is no path inside the base context: {e}"))?;
    let bytes = loader
        .load(&resource)
        .await
        .map_err(|e| format!("does not load: {e}"))?;
    if bytes.len() > MAX_FONT_OR_IMAGE_FILE_BYTES {
        return Err(format!(
            "exceeds the limit of {MAX_FONT_OR_IMAGE_FILE_BYTES} bytes for the file of a font \
             or an image"
        ));
    }
    Ok(bytes)
}
