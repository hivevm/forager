// The character of a TrueType symbol is read as the numeric entities of msGetUnicodeEntity(),
// src/mapstring.cpp of the release pinned in scripts/fetch-msautotest.sh, decode it.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

//! The port a TrueType marker asks its glyph of, which the engine answers through the render's
//! fonts and `label` (ADR-draft-truetype-marker-glyph-through-a-port).

use std::rc::Rc;

use forager_render::PathCommand;
use forager_style_ir::Symbol;

/// A glyph a marker draws: its outline in pixels about its origin, y down as the display list has
/// it, its control box in the same pixels, and whether the font lacks the character, so that
/// glyph 0 stands in for it.
#[derive(Debug, PartialEq)]
pub struct Glyph {
    pub path: Vec<PathCommand>,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub missing: bool,
}

/// The port a TrueType marker asks its glyph of: the glyph of `character` in the font the font
/// set names `font`, at `size` points, or why there is none.
pub trait Glyphs {
    fn glyph(&self, font: &str, character: char, size: u32) -> Result<Rc<Glyph>, String>;
}

/// The font a TrueType symbol names and the character of its `CHARACTER` (see [`character`]), as
/// msGetFontFace and msUTF8ToUniChar take them; or why it has none. A symbol without a `FONT` has
/// none, where MapServer draws its built-in font, which Forager does not bundle.
pub(crate) fn font_and_character(symbol: &Symbol) -> Result<(&str, char), String> {
    let font = symbol.font.as_deref().ok_or_else(|| {
        "the TrueType symbol names no FONT, where MapServer draws its built-in font, which \
         Forager does not bundle"
            .to_owned()
    })?;
    let character = character(symbol.character.as_deref().unwrap_or(""))?;
    Ok((font, character))
}

/// The character a TrueType symbol's `CHARACTER` names, as msUTF8ToUniChar reads its first: a
/// numeric entity, `&#NNN;` of up to six decimal digits or `&#xHH;` of up to five hexadecimal
/// ones, as msGetUnicodeEntity decodes it; otherwise the first character, or U+0000 for an empty
/// text, which maps to glyph 0. A named entity, `&` and up to eight characters and `;`, which
/// MapServer looks up in its table of 252, is refused, as Forager does not carry that table. A
/// numeric one that is no Unicode scalar value is refused too.
pub(crate) fn character(text: &str) -> Result<char, String> {
    if let Some(code) = numeric_entity(text) {
        return char::from_u32(code)
            .ok_or_else(|| format!("its CHARACTER `{text}` names no Unicode character"));
    }
    // msGetUnicodeEntity tries a name only where no `#` follows the `&`, of up to eight bytes.
    if let Some(rest) = text.strip_prefix('&')
        && !rest.starts_with('#')
        && let Some(end) = rest.find(';')
        && (1..=8).contains(&end)
    {
        return Err(format!(
            "its CHARACTER `{text}` starts with a named entity, which is not decoded yet; a \
             numeric one, such as `&#65;`, is"
        ));
    }
    Ok(text.chars().next().unwrap_or('\0'))
}

/// msGetUnicodeEntity's numeric entity at the start of `text`: `&#` and one to six decimal digits,
/// or `&#x` (or `&#X`) and one to five hexadecimal digits, each followed by `;`.
fn numeric_entity(text: &str) -> Option<u32> {
    let rest = text.strip_prefix("&#")?;
    let (digits, radix, most) = match rest.strip_prefix(['x', 'X']) {
        Some(hex) => (hex, 16, 5),
        None => (rest, 10, 6),
    };
    let count = digits
        .chars()
        .take(most)
        .take_while(|c| c.is_digit(radix))
        .count();
    // The digits counted are ASCII, so `count` stands on a character's boundary.
    let (number, after) = (digits.get(..count)?, digits.get(count..)?);
    if count == 0 || !after.starts_with(';') {
        return None;
    }
    u32::from_str_radix(number, radix).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_is_read_as_ms_utf8_to_uni_char_reads_it() {
        // Numeric entities, decimal and hexadecimal, as msGetUnicodeEntity decodes them.
        assert_eq!(character("&#8730;"), Ok('√'));
        assert_eq!(character("&#x221A;"), Ok('√'));
        assert_eq!(character("&#X221a;tail"), Ok('√'));
        assert_eq!(character("A"), Ok('A'));
        assert_eq!(character("√x"), Ok('√'));
        // No entity: too many digits, none, or no `;` leave the `&` itself.
        assert_eq!(character("&#1234567;"), Ok('&'));
        assert_eq!(character("&#x123456;"), Ok('&'));
        assert_eq!(character("&#;"), Ok('&'));
        assert_eq!(character("&#65"), Ok('&'));
        assert_eq!(character("&"), Ok('&'));
        // An empty CHARACTER is code point 0, which maps to glyph 0.
        assert_eq!(character(""), Ok('\0'));
        // A named entity is refused, as is a code point that is no Unicode scalar value.
        assert!(character("&radic;").unwrap_err().contains("named entity"));
        assert!(
            character("&#xD800;")
                .unwrap_err()
                .contains("no Unicode character")
        );
        // Past eight characters before `;` it is no entity to MapServer either.
        assert_eq!(character("&abcdefghi;"), Ok('&'));
    }
}
