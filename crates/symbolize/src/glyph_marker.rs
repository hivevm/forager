// The drawing of a TrueType symbol at a point follows the TrueType branch of MapServer's
// msDrawMarkerSymbolInternal() and drawGlyphMarker(), src/maprendering.c, and
// agg2RenderGlyphsPath(), src/mapagg.cpp, of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

//! A TrueType symbol at a point: its glyph asked of the port [`crate::glyph::Glyphs`], placed as drawGlyphMarker
//! places it and drawn as agg2RenderGlyphsPath draws it
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

/// Where drawGlyphMarker puts the origin of a glyph whose control box is `box_` to centre it on
/// `at`, turned by `rotation` in radians: the centre of the box, turned where the rotation is not
/// 0, taken from `at` in x and added in y, since the glyph's y runs up.
pub(crate) fn origin(
    at: Point,
    (min_x, min_y, max_x, max_y): (f64, f64, f64, f64),
    rotation: f64,
) -> Point {
    let ox = (max_x + min_x) / 2.0;
    let oy = (max_y + min_y) / 2.0;
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

/// `path`, in the font's coordinates, y up, drawn from `origin` turned by `rotation`, as
/// agg2RenderGlyphsPath transforms it: y flipped, as decompose_ft_outline flips it, then turned by
/// `-rotation` and moved to the origin.
fn placed(path: &[PathCommand], origin: Point, rotation: f64) -> Vec<PathCommand> {
    let matrix = Affine::rotation(-rotation).then(Affine::translation(origin.x, origin.y));
    let at = |p: &Point| matrix.apply(p.x, -p.y);
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
/// of draws glyph 0, and the canvas records that too. The symbol's `ANCHORPOINT` is not applied.
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
    let (Some(font), Some(character)) = (
        symbol.font.as_deref(),
        symbol.character.as_deref().and_then(|c| c.chars().next()),
    ) else {
        let why = "the TrueType symbol names no FONT or no CHARACTER".to_owned();
        canvas.refused_symbols.entry(index).or_insert(why);
        return;
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
                "the font `{font}` has no glyph for the character `{character}`; glyph 0 is drawn"
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
        // A box from (1, -2) to (11, 18): its centre (6, 8). Unturned, the origin is
        // (100 - 6, 50 + 8).
        let bounds = (1.0, -2.0, 11.0, 18.0);
        let at = Point { x: 100.0, y: 50.0 };
        assert_eq!(origin(at, bounds, 0.0), Point { x: 94.0, y: 58.0 });
        // Turned by 90 degrees, as libm gives sin and cos: rox = 6 cos - 8 sin, roy = 6 sin + 8 cos.
        let a = std::f64::consts::FRAC_PI_2;
        let (sina, cosa) = (libm::sin(a), libm::cos(a));
        let turned = origin(at, bounds, a);
        assert_eq!(turned.x, 100.0 - (6.0 * cosa - 8.0 * sina));
        assert_eq!(turned.y, 50.0 + (6.0 * sina + 8.0 * cosa));
        assert!((turned.x - 108.0).abs() < 1e-12 && (turned.y - 56.0).abs() < 1e-12);
    }

    #[test]
    fn a_glyph_s_path_is_flipped_turned_and_moved_to_its_origin() {
        let path = [
            PathCommand::MoveTo(Point { x: 0.0, y: 0.0 }),
            PathCommand::LineTo(Point { x: 10.0, y: 20.0 }),
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
        // Turned by 90 degrees counter-clockwise on the map: (10, -20) after the flip goes to
        // (-20, -10) about the origin, as AGG's rotation by -90 degrees takes it.
        let turned = placed(&path, origin, std::f64::consts::FRAC_PI_2);
        let PathCommand::LineTo(p) = turned[1] else {
            panic!("{turned:?}");
        };
        assert!(
            (p.x - 74.0).abs() < 1e-12 && (p.y - 48.0).abs() < 1e-12,
            "{p:?}"
        );
    }
}
