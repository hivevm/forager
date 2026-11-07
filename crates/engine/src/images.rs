//! The images a render's pixmap symbols name, loaded through the resource loader the first time
//! the render needs each, decoded once, and held until the render ends, as the fonts are
//! (ADR-0040, ADR-0028).

use std::rc::Rc;

use forager_encode::{decode_pixmap, pixmap_size};
use forager_ports::{ResourceLoader, ResourcePath};
use forager_render::PremultipliedImage;
use forager_symbolize::Pixmaps;

use crate::fonts::MAX_FONT_BYTES;

/// The most bytes the images one render holds decoded come to (Q-9).
pub const MAX_DECODED_IMAGE_BYTES: usize = 256 * 1024 * 1024;

/// The images of one render, by their path, each the decoded image or why it has none.
#[derive(Default)]
pub struct Images {
    loaded: Pixmaps,
    /// The bytes of the images decoded.
    held: usize,
}

impl Images {
    /// The image at `path`, loaded through `loader` and decoded the first time it is asked for;
    /// every later call takes the same image, or the same reason, naming the path, why it has
    /// none.
    pub async fn image<L: ResourceLoader>(
        &mut self,
        path: &str,
        loader: &L,
    ) -> Result<Rc<PremultipliedImage>, String> {
        if let Some(loaded) = self.loaded.get(path) {
            return loaded.clone();
        }
        let loaded = self
            .decode(path, loader)
            .await
            .map(Rc::new)
            .map_err(|why| format!("the image {path} {why}"));
        self.loaded.insert(path.to_owned(), loaded.clone());
        loaded
    }

    /// Every image asked for so far, by its path, each the image or why it has none.
    pub fn decoded(&self) -> &Pixmaps {
        &self.loaded
    }

    /// The image at `path` decoded, held to the limits of Q-9: its file to the font file's
    /// bound, which the browser bindings' loader does not hold it to; its size, read from its
    /// header, to the pixels of a symbol image and, before it is decoded, to what is left of the
    /// bytes the render's images may come to.
    async fn decode<L: ResourceLoader>(
        &mut self,
        path: &str,
        loader: &L,
    ) -> Result<PremultipliedImage, String> {
        let resource = ResourcePath::new(path)
            .map_err(|e| format!("is no path inside the base context: {e}"))?;
        let bytes = loader
            .load(&resource)
            .await
            .map_err(|e| format!("does not load: {e}"))?;
        if bytes.len() > MAX_FONT_BYTES {
            return Err(format!(
                "exceeds the limit of {MAX_FONT_BYTES} bytes for an image file"
            ));
        }
        let (width, height) = pixmap_size(&bytes).map_err(|e| e.to_string())?;
        let held = self.held + width as usize * height as usize * 4;
        if held > MAX_DECODED_IMAGE_BYTES {
            return Err(format!(
                "would bring the images this render holds decoded past the limit of \
                 {MAX_DECODED_IMAGE_BYTES} bytes"
            ));
        }
        let image = decode_pixmap(&bytes).map_err(|e| e.to_string())?;
        self.held = held;
        Ok(image)
    }
}
