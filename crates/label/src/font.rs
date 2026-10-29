// The font list follows get_face_for_run and check_single_font, src/textlayout.c, and the
// glyph index msGetGlyphIndex and the character map msGetFontFace selects, src/fontcache.c, of
// MapServer at the release pinned in scripts/fetch-msautotest.sh, for a line MapServer gives its
// basic shaper; the character map FreeType 2.13.3 selects first, find_unicode_charmap,
// src/base/ftobjs.c, with the encodings of sfnt_find_encoding and the Unicode map it builds from
// glyph names, sfnt_load_face, src/sfnt/sfobjs.c.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2013 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use skrifa::outline::OutlineGlyphFormat;
use skrifa::raw::TableProvider;
use skrifa::raw::tables::cmap::{CmapSubtable, PlatformId};
use skrifa::raw::types::Version16Dot16;
use skrifa::{FontRef, MetadataProvider};

/// The encoding FreeType gives a character map, of those MapServer reads a label through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    Unicode,
    Symbol,
    AppleRoman,
}

/// Face 0 of a font file, as MapServer opens it, with the character map it reads it through.
pub struct Font<'a> {
    pub(crate) font: FontRef<'a>,
    charmap: Option<(Encoding, CmapSubtable<'a>)>,
    num_glyphs: u32,
    /// Why no line can be read in the font, decided when it opens.
    unreadable: Option<Unusable>,
}

/// Why a font that opens cannot draw a label's line (ADR-0028).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    /// The font has no Unicode character map, and FreeType would read it through one it builds
    /// from the font's glyph names, which this crate does not build.
    NoUnicodeCharacterMap,
    /// The font's character map is of format 2 or 8, which this crate does not read.
    UnreadCharacterMapFormat,
    /// The font is read through its Symbol character map, out of the scope of ADR-0028.
    SymbolCharacterMap,
    /// The font is read through its Apple Roman character map, as it has neither a Unicode nor a
    /// Symbol one, out of the scope of ADR-0028.
    AppleRomanCharacterMap,
    /// The font's outlines are not TrueType's `glyf`.
    NoGlyfOutlines,
}

impl<'a> Font<'a> {
    /// Face 0 of the font file `data`, or why it does not open.
    pub fn new(data: &'a [u8]) -> Result<Self, String> {
        let font = FontRef::from_index(data, 0).map_err(|e| e.to_string())?;
        let num_glyphs = font.maxp().map_err(|e| e.to_string())?.num_glyphs();
        let charmap = charmap(&font);
        // sfnt_load_face builds a Unicode map from the glyph names of a face with neither a
        // Unicode nor a Symbol map whose `post` table names its glyphs, any version but 3.0.
        let named = font
            .post()
            .is_ok_and(|post| post.version() != Version16Dot16::VERSION_3_0);
        let unreadable = match &charmap {
            Some((_, CmapSubtable::Format2(_) | CmapSubtable::Format8(_))) => {
                Some(Unusable::UnreadCharacterMapFormat)
            }
            Some((Encoding::Unicode | Encoding::Symbol, _)) => None,
            Some((Encoding::AppleRoman, _)) | None if named => {
                Some(Unusable::NoUnicodeCharacterMap)
            }
            _ => None,
        };
        Ok(Self {
            font,
            charmap,
            num_glyphs: u32::from(num_glyphs),
            unreadable,
        })
    }

    /// The glyph of `c` as `msGetGlyphIndex` finds it: through a Symbol map at `c | 0xF000`,
    /// through any other as `c`, and 0 where the map has none, where the glyph lies past the
    /// font's glyphs, as `FT_Get_Char_Index` holds it, or where the font has no map.
    pub fn glyph_index(&self, c: char) -> u32 {
        let Some((encoding, subtable)) = &self.charmap else {
            return 0;
        };
        let code = match encoding {
            Encoding::Symbol => u32::from(c) | 0xF000,
            Encoding::Unicode | Encoding::AppleRoman => u32::from(c),
        };
        match subtable.map_codepoint(code) {
            Some(glyph) if glyph.to_u32() < self.num_glyphs => glyph.to_u32(),
            _ => 0,
        }
    }

    /// The glyphs of `line` as `check_single_font` finds them: `None` when the font lacks one
    /// and is not the `last` entry of the list, which takes its `?`, or glyph 0 where it has none,
    /// for each it lacks. A font that would draw the line but that this crate cannot draw it in
    /// is an error: one read through its Symbol or its Apple Roman map, and one without `glyf`
    /// outlines; a font whose glyphs cannot be found at all is an error at once.
    pub fn glyphs(&self, line: &[char], last: bool) -> Result<Option<Vec<u32>>, Unusable> {
        if let Some(unusable) = self.unreadable {
            return Err(unusable);
        }
        let mut glyphs = Vec::with_capacity(line.len());
        for &c in line {
            match self.glyph_index(c) {
                0 if !last => return Ok(None),
                0 => glyphs.push(self.glyph_index('?')),
                glyph => glyphs.push(glyph),
            }
        }
        match self.charmap {
            Some((Encoding::Symbol, _)) => return Err(Unusable::SymbolCharacterMap),
            Some((Encoding::AppleRoman, _)) => return Err(Unusable::AppleRomanCharacterMap),
            _ => {}
        }
        if self.font.outline_glyphs().format() != Some(OutlineGlyphFormat::Glyf) {
            return Err(Unusable::NoGlyfOutlines);
        }
        Ok(Some(glyphs))
    }
}

/// The character map a face is read through: the one `find_unicode_charmap` selects, the last
/// Unicode map of full repertoire, (3, 10) or (0, 4), else the last Unicode map of any kind;
/// failing both, as `msGetFontFace` selects one, the first Symbol map, else the first Apple Roman
/// one. A subtable that does not read, or of a format FreeType does not know, is no map, as
/// FreeType ignores it.
fn charmap<'a>(font: &FontRef<'a>) -> Option<(Encoding, CmapSubtable<'a>)> {
    let cmap = font.cmap().ok()?;
    let maps: Vec<(PlatformId, u16, Encoding, CmapSubtable<'a>)> = cmap
        .encoding_records()
        .iter()
        .filter_map(|record| {
            let (platform, id) = (record.platform_id(), record.encoding_id());
            let encoding = match (platform, id) {
                (PlatformId::ISO | PlatformId::Unicode, _) | (PlatformId::Windows, 1 | 10) => {
                    Encoding::Unicode
                }
                (PlatformId::Windows, 0) => Encoding::Symbol,
                (PlatformId::Macintosh, 0) => Encoding::AppleRoman,
                _ => return None,
            };
            let subtable = record.subtable(cmap.offset_data()).ok()?;
            if matches!(subtable, CmapSubtable::Format15(_)) {
                return None;
            }
            Some((platform, id, encoding, subtable))
        })
        .collect();
    let full = maps.iter().rev().find(|(platform, id, encoding, _)| {
        *encoding == Encoding::Unicode
            && matches!(
                (platform, id),
                (PlatformId::Windows, 10) | (PlatformId::Unicode, 4)
            )
    });
    let unicode = || maps.iter().rev().find(|m| m.2 == Encoding::Unicode);
    let first = |wanted| maps.iter().find(|m: &&(_, _, Encoding, _)| m.2 == wanted);
    full.or_else(unicode)
        .or_else(|| first(Encoding::Symbol))
        .or_else(|| first(Encoding::AppleRoman))
        .map(|(_, _, encoding, subtable)| (*encoding, subtable.clone()))
}

/// One step of a label's font list: the font set's names to try in turn until one opens, and
/// whether this is the last entry, which takes its `?` for a glyph it lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt<'a> {
    pub names: Vec<&'a str>,
    pub last: bool,
}

/// The steps `get_face_for_run` takes through the font list `list` for a Latin line: each entry
/// prefixed `en:` first, without the prefix, then every entry in order. Both loops stop at the
/// end of the list, so an empty entry after a trailing comma is no step and leaves the entry
/// before it not the last one; an empty entry inside the list is a step that finds no font. Each
/// step tries, as `check_single_font` does, the name after its first colon, then the name as
/// written.
pub fn attempts(list: &str) -> Vec<Attempt<'_>> {
    let mut entries = Vec::new();
    let mut rest = list;
    while !rest.is_empty() {
        match rest.split_once(',') {
            Some((entry, after)) => {
                entries.push((entry, false));
                rest = after;
            }
            None => {
                entries.push((rest, true));
                rest = "";
            }
        }
    }
    let english = entries
        .iter()
        .filter_map(|(entry, _)| entry.strip_prefix("en:"))
        .map(|name| Attempt {
            names: names(name),
            last: false,
        });
    let all = entries.iter().map(|&(entry, last)| Attempt {
        names: names(entry),
        last,
    });
    english.chain(all).collect()
}

/// The names `check_single_font` looks a font list entry up by: after its first colon, then as
/// written.
fn names(entry: &str) -> Vec<&str> {
    match entry.split_once(':') {
        Some((_, name)) => vec![name, entry],
        None => vec![entry],
    }
}
