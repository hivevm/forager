//! The fonts a render's labels name, loaded through the resource loader once per render when a
//! label first needs them, and the one a line is drawn in, chosen as `get_face_for_run` of
//! MapServer's src/textlayout.c chooses it (ADR-0028).

use std::collections::BTreeMap;
use std::rc::Rc;

use forager_label::{Font, Unusable, attempts};
use forager_ports::{ResourceLoader, ResourcePath};
use forager_style_ir::Map;

/// The largest font file a render loads (Q-9).
pub const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;

/// The fonts loaded during one render, by their path, each its bytes or why it does not open.
#[derive(Default)]
pub(crate) struct Fonts {
    loaded: BTreeMap<String, Result<Rc<[u8]>, String>>,
}

/// The font a line is drawn in, and its glyphs there.
pub(crate) struct Face {
    /// The font's path, which names it in the render.
    pub(crate) path: String,
    pub(crate) bytes: Rc<[u8]>,
    pub(crate) glyphs: Vec<u32>,
}

impl Fonts {
    /// The font of the font list `list` that draws `line`: the steps of [`attempts`] in order,
    /// each trying its names until one the font set holds, matched without regard to case as
    /// `msLookupHashTable` matches it, opens; the first font with every glyph of the line, or the
    /// last entry with its `?` for those it lacks. A font that opens but that this renderer
    /// cannot read the line in ends the search, as MapServer would have drawn with it. The error
    /// says why no font draws the line, naming the font and its path.
    pub(crate) async fn face<L: ResourceLoader>(
        &mut self,
        map: &Map,
        list: &str,
        line: &[char],
        loader: &L,
    ) -> Result<Face, String> {
        let mut why = "the label's font list names no font".to_owned();
        for attempt in attempts(list) {
            let mut opened = None;
            for name in &attempt.names {
                let Some(path) = map
                    .fonts
                    .iter()
                    .find_map(|(key, path)| key.eq_ignore_ascii_case(name).then_some(path))
                else {
                    why = format!("the font set has no font `{name}`");
                    continue;
                };
                match self.load(path, loader).await {
                    Ok(bytes) => {
                        opened = Some((name, path, bytes));
                        break;
                    }
                    Err(reason) => why = format!("the font `{name}` ({path}) {reason}"),
                }
            }
            let Some((name, path, bytes)) = opened else {
                continue;
            };
            let font = Font::new(&bytes).map_err(|e| format!("the font `{name}` ({path}): {e}"))?;
            match font.glyphs(line, attempt.last) {
                Ok(Some(glyphs)) => {
                    return Ok(Face {
                        path: path.clone(),
                        bytes,
                        glyphs,
                    });
                }
                Ok(None) => why = format!("the font `{name}` ({path}) lacks a glyph of the text"),
                Err(unusable) => {
                    let what = match unusable {
                        Unusable::NoUnicodeCharacterMap => {
                            "has no Unicode character map; FreeType would read it through one \
                             built from its glyph names, which is not built here"
                        }
                        Unusable::UnreadCharacterMapFormat => {
                            "has a character map of format 2 or 8, which is not read"
                        }
                        Unusable::SymbolCharacterMap => "has only a Symbol character map",
                        Unusable::AppleRomanCharacterMap => "has only an Apple Roman character map",
                        Unusable::NoGlyfOutlines => "has no TrueType (glyf) outlines",
                    };
                    return Err(format!("the font `{name}` ({path}) {what}"));
                }
            }
        }
        Err(format!(
            "{why}, and no other font of the label's list can be used"
        ))
    }

    /// The font at `path`, loaded once per render whatever names the font set gives it.
    async fn load<L: ResourceLoader>(
        &mut self,
        path: &str,
        loader: &L,
    ) -> Result<Rc<[u8]>, String> {
        if let Some(loaded) = self.loaded.get(path) {
            return loaded.clone();
        }
        let loaded = match ResourcePath::new(path) {
            Err(e) => Err(format!("is no path inside the base context: {e}")),
            Ok(resource) => match loader.load(&resource).await {
                Err(e) => Err(format!("does not load: {e}")),
                Ok(bytes) if bytes.len() > MAX_FONT_BYTES => Err(format!(
                    "exceeds the limit of {MAX_FONT_BYTES} bytes for a font"
                )),
                Ok(bytes) => match Font::new(&bytes) {
                    Err(e) => Err(format!("is no font: {e}")),
                    Ok(_) => Ok(Rc::from(bytes)),
                },
            },
        };
        self.loaded.insert(path.to_owned(), loaded.clone());
        loaded
    }
}
