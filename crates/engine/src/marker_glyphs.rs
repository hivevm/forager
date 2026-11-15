//! The engine's answer to the glyph port of a TrueType marker
//! (ADR-draft-truetype-marker-glyph-through-a-port): the fonts the render's TrueType symbols name,
//! loaded before a layer is drawn into the store the labels' fonts use, and each glyph read
//! through `label` once per render by its font, character, and size, as MapServer's glyph cache
//! holds it per face and size.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use forager_label::{Font, marker_glyph};
use forager_symbolize::{Glyph, Glyphs};

/// A font kept for the markers: its path and its bytes, or why it cannot be used.
pub(crate) type MarkerFont = Result<(String, Rc<[u8]>), String>;

/// A glyph read for the markers, or why there is none.
type Read = Result<Rc<Glyph>, String>;

/// The fonts of one render's TrueType markers, by the name the font set gives them, and the
/// glyphs read from them, by the font's name, the character, and the size in points.
#[derive(Default)]
pub(crate) struct MarkerGlyphs {
    fonts: BTreeMap<String, MarkerFont>,
    read: RefCell<BTreeMap<(String, char, u32), Read>>,
}

impl MarkerGlyphs {
    /// Whether the font of `name` has been looked for in this render.
    pub(crate) fn has(&self, name: &str) -> bool {
        self.fonts.contains_key(name)
    }

    /// Keeps the font of `name`, or why it cannot be used.
    pub(crate) fn keep(&mut self, name: &str, font: MarkerFont) {
        self.fonts.insert(name.to_owned(), font);
    }
}

impl Glyphs for MarkerGlyphs {
    fn glyph(&self, font: &str, character: char, size: u32) -> Result<Rc<Glyph>, String> {
        let key = (font.to_owned(), character, size);
        if let Some(read) = self.read.borrow().get(&key) {
            return read.clone();
        }
        let read = match self.fonts.get(font) {
            None => Err(format!("the font `{font}` is not loaded for this render")),
            Some(Err(why)) => Err(why.clone()),
            Some(Ok((path, bytes))) => Font::new(bytes)
                .map_err(|e| format!("the font `{font}` ({path}): {e}"))
                .and_then(|face| {
                    marker_glyph(&face, character, f64::from(size)).ok_or_else(|| {
                        format!(
                            "the font `{font}` ({path}) cannot draw the glyph of `{character}`, \
                             nor its `?`"
                        )
                    })
                })
                .map(|glyph| {
                    Rc::new(Glyph {
                        path: glyph.path,
                        min_x: glyph.min_x,
                        min_y: glyph.min_y,
                        max_x: glyph.max_x,
                        max_y: glyph.max_y,
                        missing: glyph.missing,
                    })
                }),
        };
        self.read.borrow_mut().insert(key, read.clone());
        read
    }
}
