// The font list follows get_face_for_run and check_single_font, src/textlayout.c, and the
// glyph index msGetGlyphIndex, src/fontcache.c, of MapServer at the release pinned in
// scripts/fetch-msautotest.sh, for a line MapServer gives its basic shaper.
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
use skrifa::{FontRef, MetadataProvider, charmap::Charmap};

/// Face 0 of a font file, as MapServer opens it.
pub struct Font<'a> {
    pub(crate) font: FontRef<'a>,
    charmap: Charmap<'a>,
}

/// Why a font that opens cannot draw a label's line (ADR-0028).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    /// The font has no Unicode character map; MapServer would read its Symbol or Apple Roman map.
    NoUnicodeCharacterMap,
    /// The font's outlines are not TrueType's `glyf`.
    NoGlyfOutlines,
}

impl<'a> Font<'a> {
    /// Face 0 of the font file `data`, or why it does not open.
    pub fn new(data: &'a [u8]) -> Result<Self, String> {
        let font = FontRef::from_index(data, 0).map_err(|e| e.to_string())?;
        let charmap = font.charmap();
        Ok(Self { font, charmap })
    }

    /// The glyph of `c` in the font's Unicode character map, 0 where it has none.
    pub fn glyph_index(&self, c: char) -> u32 {
        self.charmap.map(c).map_or(0, |g| g.to_u32())
    }

    /// The glyphs of `line` as `check_single_font` finds them: `None` when the font lacks one
    /// and is not the `last` entry of the list, which takes its `?`, or glyph 0 where it has none,
    /// for each it lacks. A font this crate cannot read the line in is an error once it would
    /// be used: one without a Unicode character map at once, as its glyphs cannot be found, and
    /// one without `glyf` outlines once it has them.
    pub fn glyphs(&self, line: &[char], last: bool) -> Result<Option<Vec<u32>>, Unusable> {
        if !self.charmap.has_map() || self.charmap.is_symbol() {
            return Err(Unusable::NoUnicodeCharacterMap);
        }
        let mut glyphs = Vec::with_capacity(line.len());
        for &c in line {
            match self.glyph_index(c) {
                0 if !last => return Ok(None),
                0 => glyphs.push(self.glyph_index('?')),
                glyph => glyphs.push(glyph),
            }
        }
        if self.font.outline_glyphs().format() != Some(OutlineGlyphFormat::Glyf) {
            return Err(Unusable::NoGlyfOutlines);
        }
        Ok(Some(glyphs))
    }
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
