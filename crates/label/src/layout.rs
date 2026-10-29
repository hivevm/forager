// The layout follows msLayoutTextSymbol and its basic shaper and unicode_is_latin1,
// src/textlayout.c, the glyph metrics msGetGlyphByIndex, src/fontcache.c, and the label's point at
// POSITION CC get_metrics, src/maplabel.c, with msDrawTextSymbol, src/maprendering.c, of MapServer
// at the release pinned in scripts/fetch-msautotest.sh.
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

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use forager_render::{PathCommand, Point};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::DrawSettings;
use skrifa::outline::pen::{OutlinePen, PathStyle};
use skrifa::{GlyphId, MetadataProvider};

use crate::font::Font;
use crate::size::pixel_size;

/// Why a label's text is not laid out as one Latin line (ADR-0028).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineError {
    /// The text breaks into several lines, which `WRAP` and `ALIGN` lay out.
    SeveralLines,
    /// A code point at or above U+02B0, which MapServer shapes with HarfBuzz.
    NotLatin(char),
}

/// The characters of a label's text as `msLayoutTextSymbol` reads them, each `\r` left out, when
/// they make one line that `unicode_is_latin1` gives the basic shaper; empty when there are none.
pub fn line(text: &str) -> Result<Vec<char>, LineError> {
    let chars: Vec<char> = text.chars().filter(|&c| c != '\r').collect();
    // A last `\n` closes the line without opening another.
    let body = chars.strip_suffix(&['\n']).unwrap_or(&chars);
    if body.contains(&'\n') {
        return Err(LineError::SeveralLines);
    }
    if let Some(&c) = body.iter().find(|&&c| u32::from(c) >= 0x2B0) {
        return Err(LineError::NotLatin(c));
    }
    Ok(body.to_vec())
}

/// A glyph drawn at a pixel size: its outline in pixels, y up from its origin, the control box of
/// its points, and its advance, as FreeType gives them unhinted.
#[derive(Debug)]
struct Glyph {
    path: Vec<PathCommand>,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    advance: f64,
}

/// A line of glyphs laid out from its origin on the baseline, with its bounding box, as
/// MapServer's `textPathObj` holds it: x to the right, y down.
#[derive(Debug)]
pub struct TextPath {
    /// Each glyph's pen position along the baseline, and the glyph.
    placed: Vec<(f64, u32)>,
    glyphs: BTreeMap<u32, Glyph>,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl TextPath {
    /// Lays out `glyphs` of `font` at `glyph_size` as the basic shaper does: each at the pen,
    /// which moves by the glyph's advance, with no kerning. A glyph that cannot be drawn takes
    /// the font's `?`, or glyph 0, as `msGetGlyphByIndex` falls back; the error names one that
    /// cannot be drawn either way.
    pub fn new(font: &Font<'_>, glyphs: &[u32], glyph_size: f64) -> Result<Self, u32> {
        let ppem = pixel_size(glyph_size);
        let mut path = TextPath {
            placed: Vec::with_capacity(glyphs.len()),
            glyphs: BTreeMap::new(),
            min_x: 0.0,
            // FLT_MAX, as initialised in msLayoutTextSymbol.
            min_y: f64::from(f32::MAX),
            max_x: -f64::from(f32::MAX),
            max_y: -f64::from(f32::MAX),
        };
        let mut pen_x = 0.0;
        // The one line's pen height, (1 - numlines + line_number) * line_height.
        let pen_y = 0.0;
        for &id in glyphs {
            let glyph = match path.glyphs.entry(id) {
                Entry::Occupied(drawn) => drawn.into_mut(),
                Entry::Vacant(slot) => slot.insert(
                    draw(font, id, ppem)
                        .or_else(|| draw(font, font.glyph_index('?'), ppem))
                        .ok_or(id)?,
                ),
            };
            path.placed.push((pen_x, id));
            pen_x += glyph.advance;
            if pen_y - glyph.max_y < path.min_y {
                path.min_y = pen_y - glyph.max_y;
            }
            if pen_y - glyph.min_y > path.max_y {
                path.max_y = pen_y - glyph.min_y;
            }
        }
        if pen_x > path.max_x {
            path.max_x = pen_x;
        }
        Ok(path)
    }

    /// The point the line is drawn from when it is centred on `at`, as `get_metrics` places a
    /// label at `POSITION CC` without offset or angle.
    pub fn centred_on(&self, at: Point) -> Point {
        let w = self.max_x - self.min_x;
        let h = self.max_y - self.min_y;
        let x1 = -(w / 2.0) + 0.0;
        let y1 = (h / 2.0) + 0.0;
        Point {
            x: at.x + x1 - self.min_x,
            y: at.y + y1 - self.max_y,
        }
    }

    /// The outlines of the glyphs drawn from `origin`, in pixels, as `agg2RenderGlyphsPath` turns
    /// them into one path: each glyph's y flipped and moved to its point. A glyph whose box lies
    /// wholly outside `visible`, `[min x, min y, max x, max y]`, adds nothing, so that what a
    /// line costs to draw follows what of it can be seen.
    pub fn draw(&self, origin: Point, visible: [f64; 4]) -> Vec<PathCommand> {
        let mut out = Vec::new();
        for &(pen_x, id) in &self.placed {
            let glyph = &self.glyphs[&id];
            // msDrawTextSymbol moves each glyph's point by the label's; agg2RenderGlyphsPath
            // then translates the flipped outline to it.
            let gx = pen_x + origin.x;
            let gy = 0.0 + origin.y;
            let seen = gx + glyph.max_x >= visible[0]
                && gx + glyph.min_x <= visible[2]
                && gy - glyph.min_y >= visible[1]
                && gy - glyph.max_y <= visible[3];
            if !seen {
                continue;
            }
            let at = |p: Point| Point {
                x: p.x + gx,
                y: -p.y + gy,
            };
            out.extend(glyph.path.iter().map(|&command| match command {
                PathCommand::MoveTo(p) => PathCommand::MoveTo(at(p)),
                PathCommand::LineTo(p) => PathCommand::LineTo(at(p)),
                PathCommand::QuadTo(c, p) => PathCommand::QuadTo(at(c), at(p)),
                PathCommand::CubicTo(c0, c1, p) => PathCommand::CubicTo(at(c0), at(c1), at(p)),
                PathCommand::Close => PathCommand::Close,
            }));
        }
        out
    }
}

/// Glyph `id` of `font` drawn unhinted at `ppem` by skrifa's FreeType path style, at the default
/// instance, or `None` where the font cannot draw it.
fn draw(font: &Font<'_>, id: u32, ppem: u16) -> Option<Glyph> {
    let outline = font.font.outline_glyphs().get(GlyphId::new(id))?;
    let settings = DrawSettings::unhinted(Size::new(f32::from(ppem)), LocationRef::default())
        .with_path_style(PathStyle::FreeType);
    let mut pen = Pen::default();
    let metrics = outline.draw(settings, &mut pen).ok()?;
    let advance = f64::from(metrics.advance_width?);
    // FT_Outline_Get_CBox of no points is all zero.
    let [min_x, min_y, max_x, max_y] = pen.cbox.unwrap_or_default();
    Some(Glyph {
        path: pen.path,
        min_x,
        min_y,
        max_x,
        max_y,
        advance,
    })
}

/// Collects a glyph's path, each coordinate a 26.6 value divided by 64, which `f32` holds
/// exactly, and the control box of every point it is given.
#[derive(Default)]
struct Pen {
    path: Vec<PathCommand>,
    cbox: Option<[f64; 4]>,
}

impl Pen {
    fn point(&mut self, x: f32, y: f32) -> Point {
        let (x, y) = (f64::from(x), f64::from(y));
        let b = self.cbox.get_or_insert([x, y, x, y]);
        *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        Point { x, y }
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.path.push(PathCommand::MoveTo(p));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.path.push(PathCommand::LineTo(p));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let c = self.point(cx0, cy0);
        let p = self.point(x, y);
        self.path.push(PathCommand::QuadTo(c, p));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let c0 = self.point(cx0, cy0);
        let c1 = self.point(cx1, cy1);
        let p = self.point(x, y);
        self.path.push(PathCommand::CubicTo(c0, c1, p));
    }

    fn close(&mut self) {
        self.path.push(PathCommand::Close);
    }
}
