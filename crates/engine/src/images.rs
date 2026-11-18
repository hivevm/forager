//! The images a render's pixmap symbols name, loaded through the resource loader the first time
//! the render needs each by the load the fonts share (`resource.rs`), decoded once, and held until
//! the render ends (ADR-0029, ADR-0031).

use std::rc::Rc;

use forager_encode::{decode_pixmap, pixmap_size};
use forager_ports::ResourceLoader;
use forager_render::PremultipliedImage;
use forager_symbolize::{MAX_DECODED_IMAGE_BYTES, Pixmaps, Tiles};

use crate::resource::load_once;

/// The images of one render, by their path, each the decoded image or why it has none, and the
/// tiles of its polygons' fills, whose bytes count with the images' (ADR-0029).
#[derive(Default)]
pub(crate) struct Images {
    loaded: Pixmaps,
    /// The bytes of the images decoded.
    held: usize,
    tiles: Tiles,
}

impl Images {
    /// The image at `path`, loaded through `loader` and decoded the first time it is asked for;
    /// every later call takes the same image, or the same reason, naming the path, why it has
    /// none.
    pub(crate) async fn image<L: ResourceLoader>(
        &mut self,
        path: &str,
        loader: &L,
    ) -> Result<Rc<PremultipliedImage>, String> {
        let (held, tiles) = (&mut self.held, &self.tiles);
        load_once(&mut self.loaded, path, loader, |file| {
            file.and_then(|bytes| decode(&bytes, held, tiles))
                .map(Rc::new)
                .map_err(|why| format!("the image {path} {why}"))
        })
        .await
    }

    /// Every image asked for so far, by its path, each the image or why it has none.
    pub(crate) fn decoded(&self) -> &Pixmaps {
        &self.loaded
    }

    /// What a layer is drawn from: every image asked for so far, and the tiles, which count those
    /// images' bytes with their own.
    pub(crate) fn drawing(&mut self) -> (&Pixmaps, &mut Tiles) {
        self.tiles.count_images(self.held);
        (&self.loaded, &mut self.tiles)
    }
}

/// The image of the file `bytes` decoded, its size, read from its header, held to the pixels of a
/// symbol image and, before it is decoded, to what is left of the bytes the render's images and
/// `tiles` may come to beyond the `held` bytes of its images, which count it once it is decoded.
fn decode(bytes: &[u8], held: &mut usize, tiles: &Tiles) -> Result<PremultipliedImage, String> {
    let (width, height) = pixmap_size(bytes).map_err(|e| e.to_string())?;
    let after = *held + width as usize * height as usize * 4;
    if after + tiles.bytes() > MAX_DECODED_IMAGE_BYTES {
        return Err(format!(
            "would bring the images and tiles this render holds decoded past the limit of \
             {MAX_DECODED_IMAGE_BYTES} bytes"
        ));
    }
    let image = decode_pixmap(bytes).map_err(|e| e.to_string())?;
    *held = after;
    Ok(image)
}
