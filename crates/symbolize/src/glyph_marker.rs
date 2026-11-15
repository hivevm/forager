// The drawing of a TrueType symbol at a point follows the TrueType branch of MapServer's
// msDrawMarkerSymbolInternal() and drawGlyphMarker(), src/maprendering.c,
// agg2RenderGlyphsPath(), src/mapagg.cpp, and the numeric entities of msGetUnicodeEntity(),
// src/mapstring.cpp, of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
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

//! A TrueType symbol at a point: its glyph asked of the port [`crate::glyph::Glyphs`], placed as
//! drawGlyphMarker places it and drawn as agg2RenderGlyphsPath draws it
//! (ADR-draft-truetype-marker-glyph-through-a-port).

use forager_render::{DrawOp, GlyphOutline, PathCommand, Point};
use forager_style_ir::c::lrint;
use forager_style_ir::{Style, Symbol};

use crate::canvas::Canvas;
use crate::symbol::{self, Affine};

/// The size in points a TrueType marker's glyph is taken at: `MS_MAX(MS_NINT(scale), 1)`, held to
/// an `unsigned int`, as msDrawMarkerSymbolInternal hands it to msGetGlyphByIndex.
fn glyph_size(scale: f64) -> u32 {
    let rounded = lrint(scale).max(1);
    u32::try_from(rounded).unwrap_or(u32::MAX)
}

/// The character a TrueType symbol's `CHARACTER` names, as msUTF8ToUniChar reads its first: a
/// numeric entity, `&#NNN;` of up to six decimal digits or `&#xHH;` of up to five hexadecimal
/// ones, as msGetUnicodeEntity decodes it; otherwise the first character, or U+0000 for an empty
/// text, which maps to glyph 0. A named entity, `&` and up to eight characters and `;`, which
/// MapServer looks up in its table of 252, is refused, as Forager does not carry that table. A
/// numeric one that is no Unicode scalar value is refused too.
fn character(text: &str) -> Result<char, String> {
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

/// Where drawGlyphMarker puts the origin of a glyph whose control box, y down, is `bounds`, to
/// centre it on `at`, turned by `rotation` in radians: the centre of the box, turned where the
/// rotation is not 0. drawGlyphMarker computes the centre in the font's coordinates, y up, takes
/// it from `at` in x and adds it in y; its y is the negated centre of the box y down, exactly.
pub(crate) fn origin(
    at: Point,
    (min_x, min_y, max_x, max_y): (f64, f64, f64, f64),
    rotation: f64,
) -> Point {
    let ox = (max_x + min_x) / 2.0;
    let oy = -((max_y + min_y) / 2.0);
    if rotation != 0.0 {
        let (sina, cosa) = (libm::sin(rotation), libm::cos(rotation));
        let rox = ox * cosa - oy * sina;
        let roy = ox * sina + oy * cosa;
        Point {
            x: at.x - rox,
            y: at.y + roy,
        }
    } else {
        Point {
            x: at.x - ox,
            y: at.y + oy,
        }
    }
}

/// `path`, in pixels about the glyph's origin, y down, as the port gives it, already flipped as
/// decompose_ft_outline flips it, drawn from `origin` turned by `rotation`, as
/// agg2RenderGlyphsPath transforms it: turned by `-rotation` and moved to the origin.
fn placed(path: &[PathCommand], origin: Point, rotation: f64) -> Vec<PathCommand> {
    let matrix = Affine::rotation(-rotation).then(Affine::translation(origin.x, origin.y));
    let at = |p: &Point| matrix.apply(p.x, p.y);
    path.iter()
        .map(|command| match command {
            PathCommand::MoveTo(p) => PathCommand::MoveTo(at(p)),
            PathCommand::LineTo(p) => PathCommand::LineTo(at(p)),
            PathCommand::QuadTo(c, p) => PathCommand::QuadTo(at(c), at(p)),
            PathCommand::CubicTo(c0, c1, p) => PathCommand::CubicTo(at(c0), at(c1), at(p)),
            PathCommand::Close => PathCommand::Close,
        })
        .collect()
}

/// The TrueType symbol `symbol`, at `index` from 1, at the pixel `at` in `style` at the canvas's
/// factors, as msDrawMarkerSymbolInternal draws it: computeSymbolStyle's style, nothing without a
/// colour or at a scale of 0; moved by the style's offsets; its glyph asked of the canvas's port at
/// [`glyph_size`] and placed by [`origin`]; drawn as one glyph item, its outline the
/// `OUTLINECOLOR` at the outline width cut to an integer plus 1, its fill the `COLOR`. A glyph the
/// port has none for draws nothing, and the canvas records why; one the font lacks the character
/// of draws glyph 0, and the canvas records that too. A symbol without a `FONT` draws nothing,
/// where MapServer draws its built-in font, which Forager does not bundle; one whose `CHARACTER`
/// is refused draws nothing (see [`character`]). The symbol's `ANCHORPOINT` is not applied.
pub(crate) fn glyph_ops<T>(
    ops: &mut Vec<DrawOp>,
    at: Point,
    style: &Style<T>,
    index: usize,
    symbol: &Symbol,
    canvas: &mut Canvas,
) {
    let s = symbol::symbol_style(style, symbol, canvas.factors);
    if (s.fill.is_none() && s.outline.is_none()) || s.scale == 0.0 {
        return;
    }
    let at = symbol::shifted(at, style, canvas.factors.scale);
    let Some(font) = symbol.font.as_deref() else {
        let why = "the TrueType symbol names no FONT, where MapServer draws its built-in font, \
                   which Forager does not bundle"
            .to_owned();
        canvas.refused_symbols.entry(index).or_insert(why);
        return;
    };
    let character = match character(symbol.character.as_deref().unwrap_or("")) {
        Ok(character) => character,
        Err(why) => {
            canvas.refused_symbols.entry(index).or_insert(why);
            return;
        }
    };
    let glyph = match canvas.glyphs {
        Some(glyphs) => glyphs.glyph(font, character, glyph_size(s.scale)),
        None => Err("no fonts are kept where it is drawn".to_owned()),
    };
    let glyph = match glyph {
        Ok(glyph) => glyph,
        Err(why) => {
            canvas.refused_symbols.entry(index).or_insert(why);
            return;
        }
    };
    if glyph.missing {
        canvas.missing_glyphs.entry(index).or_insert_with(|| {
            format!(
                "the font `{font}` has no glyph for the character U+{:04X}; glyph 0 is drawn, or \
                 the font's `?` where glyph 0 cannot be",
                u32::from(character)
            )
        });
    }
    let bounds = (glyph.min_x, glyph.min_y, glyph.max_x, glyph.max_y);
    let origin = origin(at, bounds, s.rotation);
    // drawGlyphMarker's `int olwidth`: the outline width cut toward zero.
    let outline = s.outline.map(|color| GlyphOutline {
        color,
        width: s.outline_width.trunc() + 1.0,
    });
    ops.push(DrawOp::Glyphs {
        path: placed(&glyph.path, origin, s.rotation),
        color: s.fill,
        outline,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_s_size_is_its_scale_rounded_half_to_even_and_at_least_1() {
        assert_eq!(glyph_size(15.0), 15);
        assert_eq!(glyph_size(15.5), 16);
        assert_eq!(glyph_size(16.5), 16);
        assert_eq!(glyph_size(0.2), 1);
        // Past a long, lrint gives LONG_MIN, which MS_MAX lifts to 1, as in C; MAXSIZE keeps a
        // style's scale far below it.
        assert_eq!(glyph_size(1e30), 1);
    }

    #[test]
    fn a_glyph_is_centred_on_its_point_by_its_control_box_as_draw_glyph_marker_centres_it() {
        // Verifies: ADR-draft-truetype-marker-glyph-through-a-port
        // A box from (1, -2) to (11, 18) in the font's coordinates, y up, is (1, -18) to (11, 2)
        // y down. drawGlyphMarker's centre is (6, 8); unturned, the origin is (100 - 6, 50 + 8).
        let bounds = (1.0, -18.0, 11.0, 2.0);
        let at = Point { x: 100.0, y: 50.0 };
        assert_eq!(origin(at, bounds, 0.0), Point { x: 94.0, y: 58.0 });
        // Turned by 45 degrees: rox = 6 cos - 8 sin = -√2 and roy = 6 sin + 8 cos = 7√2, so the
        // origin is (100 + √2, 50 + 7√2).
        let turned = origin(at, bounds, std::f64::consts::FRAC_PI_4);
        let root2 = std::f64::consts::SQRT_2;
        assert!((turned.x - (100.0 + root2)).abs() < 1e-12, "{turned:?}");
        assert!(
            (turned.y - (50.0 + 7.0 * root2)).abs() < 1e-12,
            "{turned:?}"
        );
        // Turned by 90 degrees: rox = -8, roy = 6, the origin (108, 56).
        let turned = origin(at, bounds, std::f64::consts::FRAC_PI_2);
        assert!((turned.x - 108.0).abs() < 1e-12 && (turned.y - 56.0).abs() < 1e-12);
    }

    #[test]
    fn a_glyph_s_path_is_turned_and_moved_to_its_origin() {
        // The path y down, as the port gives it: (10, 20) up in the font is (10, -20).
        let path = [
            PathCommand::MoveTo(Point { x: 0.0, y: 0.0 }),
            PathCommand::LineTo(Point { x: 10.0, y: -20.0 }),
            PathCommand::Close,
        ];
        let origin = Point { x: 94.0, y: 58.0 };
        let unturned = placed(&path, origin, 0.0);
        assert_eq!(
            unturned,
            [
                PathCommand::MoveTo(origin),
                PathCommand::LineTo(Point { x: 104.0, y: 38.0 }),
                PathCommand::Close,
            ]
        );
        // Turned by 90 degrees counter-clockwise on the map: (10, -20) goes to (-20, -10) about
        // the origin, as AGG's rotation by -90 degrees takes it.
        let turned = placed(&path, origin, std::f64::consts::FRAC_PI_2);
        let PathCommand::LineTo(p) = turned[1] else {
            panic!("{turned:?}");
        };
        assert!(
            (p.x - 74.0).abs() < 1e-12 && (p.y - 48.0).abs() < 1e-12,
            "{p:?}"
        );
    }

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
