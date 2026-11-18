//! What one render reads through the host's resource loader, held in one place from its first use
//! until the render ends: the fonts of its labels and TrueType markers (`fonts.rs`, ADR-0031,
//! ADR-0044), the images of its pixmap symbols and the tiles of its fills (`images.rs`,
//! ADR-0029), and the TrueType markers' glyphs (`marker_glyphs.rs`).

use forager_ports::ResourceLoader;
use forager_style_ir::Map;
use forager_symbolize::{Pixmaps, Tiles};

use crate::fonts::Fonts;
use crate::images::Images;
use crate::marker_glyphs::MarkerGlyphs;

/// The resources of one render, each loaded through `loader`.
pub(crate) struct Resources<'r, L> {
    loader: &'r L,
    fonts: Fonts<'r, L>,
    images: Images,
    glyphs: MarkerGlyphs,
}

/// What a layer is drawn and its labels are laid out from, lent by [`Resources::lend`].
pub(crate) struct Lent<'a, 'r, L> {
    /// The tiles of the fills, which count the images' bytes with their own.
    pub(crate) tiles: &'a mut Tiles,
    /// The TrueType markers' glyphs.
    pub(crate) glyphs: &'a MarkerGlyphs,
    /// The images and the fonts.
    pub(crate) labels: LabelResources<'a, 'r, L>,
}

/// What a layer's labels are laid out from, and the canvas draws its pixmaps from.
pub(crate) struct LabelResources<'a, 'r, L> {
    /// Every image asked for so far, which size the markers a label moves away from or draws.
    pub(crate) pixmaps: &'a Pixmaps,
    /// The fonts, which a label's text is laid out in.
    pub(crate) fonts: &'a mut Fonts<'r, L>,
}

impl<'r, L: ResourceLoader> Resources<'r, L> {
    /// No resource yet, each to be loaded through `loader`.
    pub(crate) fn new(loader: &'r L) -> Self {
        Self {
            loader,
            fonts: Fonts::new(loader),
            images: Images::default(),
            glyphs: MarkerGlyphs::default(),
        }
    }

    /// Loads and decodes the image at `path` unless it has been asked for already; the image, or
    /// why it has none, is kept for the canvas and the labels.
    pub(crate) async fn load_pixmap(&mut self, path: &str) {
        if !self.images.decoded().contains_key(path) {
            let _ = self.images.image(path, self.loader).await;
        }
    }

    /// Keeps among the glyphs each of `fonts` the TrueType markers of a layer name and that they
    /// do not hold yet, loaded into the store of the labels' fonts (see [`Fonts::named`]), or why
    /// it cannot be used.
    pub(crate) async fn load_marker_fonts(&mut self, map: &Map, fonts: &[&str]) {
        for &name in fonts {
            if !self.glyphs.has(name) {
                let font = self.fonts.named(map, name).await;
                self.glyphs.keep(name, font);
            }
        }
    }

    /// Every image asked for so far, by its path, each the image or why it has none.
    pub(crate) fn images(&self) -> &Pixmaps {
        self.images.decoded()
    }

    /// What a layer is drawn and its labels are laid out from.
    pub(crate) fn lend(&mut self) -> Lent<'_, 'r, L> {
        let (pixmaps, tiles) = self.images.drawing();
        Lent {
            tiles,
            glyphs: &self.glyphs,
            labels: LabelResources {
                pixmaps,
                fonts: &mut self.fonts,
            },
        }
    }
}
