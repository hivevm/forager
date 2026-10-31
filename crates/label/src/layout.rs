// The layout follows msLayoutTextSymbol, its basic shaper, its lines and their alignment, and
// unicode_is_latin1, src/textlayout.c, the line height msComputeTextPath, src/maplabel.c, the
// glyph metrics msGetGlyphByIndex, src/fontcache.c, the glyphs' points msDrawTextSymbol,
// src/maprendering.c, and the glyphs' transformation agg2RenderGlyphsPath, src/mapagg.cpp, of
// MapServer at the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
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

use forager_render::{MAX_GLYPH_REACH, PathCommand, Point};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::DrawSettings;
use skrifa::outline::pen::{OutlinePen, PathStyle};
use skrifa::{GlyphId, MetadataProvider};

use crate::font::Font;
use crate::place::TextBox;
use crate::size::pixel_size;

/// Why a label's text is not laid out as Latin lines (ADR-0028).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineError {
    /// A code point at or above U+02B0, which MapServer shapes with HarfBuzz.
    NotLatin(char),
}

/// The lines of a label's text as `msLayoutTextSymbol` reads them: each `\r` left out, its wrap
/// applied, and the text broken at each `\n`, a last one closing its line without opening
/// another; none when no character is left. Without a `max_length`, each `wrap` character becomes
/// a line break. With a positive `max_length` (`MAXLENGTH`), a text of more characters than it is
/// broken at a `wrap` character or U+200B met once the line holds `max_length` characters, counted
/// as MapServer counts them, a `\n` of the text among them and resetting nothing; a text no longer
/// is laid out as written, its `wrap` characters kept. MapServer holds `WRAP` in a C `char` and
/// compares it with code points as an `unsigned int`: a character outside ASCII, its first byte
/// negative on the x86-64 build the references come from, never matches, and NUL is no wrap at
/// all. Each line must lie below U+02B0, where `unicode_is_latin1` gives it the basic shaper.
pub fn lines(
    text: &str,
    wrap: Option<char>,
    max_length: Option<usize>,
) -> Result<Vec<Vec<char>>, LineError> {
    let wrap = wrap.filter(|&c| c.is_ascii() && c != '\0');
    let mut chars: Vec<char> = text.chars().filter(|&c| c != '\r').collect();
    match max_length.filter(|&m| m > 0) {
        None => {
            for c in &mut chars {
                if Some(*c) == wrap {
                    *c = '\n';
                }
            }
        }
        Some(max) if chars.len() > max => {
            let mut on_line = 0;
            for c in &mut chars {
                if (Some(*c) == wrap || *c == '\u{200B}') && on_line >= max {
                    *c = '\n';
                    on_line = 0;
                } else {
                    on_line += 1;
                }
            }
        }
        Some(_) => {}
    }
    if chars.is_empty() {
        return Ok(Vec::new());
    }
    // A last `\n` closes the line without opening another.
    let body = chars.strip_suffix(&['\n']).unwrap_or(&chars);
    let lines: Vec<Vec<char>> = body.split(|&c| c == '\n').map(<[char]>::to_vec).collect();
    if let Some(&c) = lines.iter().flatten().find(|&&c| u32::from(c) >= 0x2B0) {
        return Err(LineError::NotLatin(c));
    }
    Ok(lines)
}

/// How the lines of a text stand beside each other, `msLayoutTextSymbol`'s `ALIGN` for lines
/// read left to right, which its default and an unknown value leave at the left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
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

/// The glyphs of one font drawn at one pixel size, each once, as `msGetGlyphByIndex` caches them
/// per face and size; a render keeps one for each font and size its labels use.
#[derive(Debug)]
pub struct Glyphs {
    ppem: u16,
    drawn: BTreeMap<u32, Glyph>,
}

impl Glyphs {
    /// An empty cache for the glyphs of one font at `glyph_size`.
    pub fn new(glyph_size: f64) -> Self {
        Self {
            ppem: pixel_size(glyph_size),
            drawn: BTreeMap::new(),
        }
    }

    /// Glyph `id` of `font`, drawn on first use; a glyph that cannot be drawn takes the font's
    /// `?`, or glyph 0, as `msGetGlyphByIndex` falls back, and `None` names one that cannot be
    /// drawn either way.
    fn get(&mut self, font: &Font<'_>, id: u32) -> Option<&Glyph> {
        match self.drawn.entry(id) {
            Entry::Occupied(drawn) => Some(drawn.into_mut()),
            Entry::Vacant(slot) => {
                let glyph = draw(font, id, self.ppem)
                    .or_else(|| draw(font, font.glyph_index('?'), self.ppem))?;
                Some(slot.insert(glyph))
            }
        }
    }
}

/// One line of a text path with glyphs: the [`Glyphs`] it was laid out with, by the index the
/// caller gave it, the end of its glyphs among the path's, and its pen's start, after the line's
/// alignment, from the path's origin.
#[derive(Debug)]
struct Line {
    cache: u32,
    end: u32,
    x: f64,
    y: f64,
}

/// A text of one or more lines laid out from its origin, the baseline of its last line, with its
/// bounding box, as MapServer's `textPathObj` holds it: x to the right, y down. It holds the
/// glyphs by their index; their outlines stay in the [`Glyphs`] each line was laid out with.
#[derive(Debug)]
pub struct TextPath {
    ids: Box<[u32]>,
    lines: Box<[Line]>,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    /// The count of lines, those without glyphs among them.
    pub num_lines: usize,
}

/// The outlines of a text path drawn at a point, and how many of its glyphs were left out for
/// reaching farther from the visible area than [`MAX_GLYPH_REACH`].
#[derive(Debug, Default, PartialEq)]
pub struct Drawn {
    pub path: Vec<PathCommand>,
    pub out_of_reach: usize,
}

/// A text path being laid out line by line, as `msLayoutTextSymbol` lays out one run per line
/// with its basic shaper.
#[derive(Debug)]
pub struct Layout {
    line_height: i64,
    num_lines: usize,
    ids: Vec<u32>,
    /// Each line with glyphs.
    lines: Vec<Line>,
    /// The length of each line, held in an `int` as `line_desc` holds it.
    lengths: Vec<i32>,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Layout {
    /// A layout of `num_lines` lines at `glyph_size`, whose lines stand `ceil(glyph_size × 1.33)`
    /// apart, as `msComputeTextPath` sets the line height, in an `int`.
    pub fn new(glyph_size: f64, num_lines: usize) -> Self {
        Self {
            line_height: (glyph_size.trunc() * 1.33).ceil() as i64,
            num_lines,
            ids: Vec::new(),
            lines: Vec::new(),
            lengths: Vec::with_capacity(num_lines),
            // FLT_MAX, as initialised in msLayoutTextSymbol.
            min_y: f64::from(f32::MAX),
            max_x: -f64::from(f32::MAX),
            max_y: -f64::from(f32::MAX),
        }
    }

    /// Lays out the next line, `glyphs` of `font`, whose glyphs `cache` holds at the label's
    /// size and which the caller names `index`, as the basic shaper does: each at the pen, which
    /// moves by the glyph's advance, with no kerning. A line without glyphs needs no font. The
    /// error names a glyph the font cannot draw, nor its fallback.
    pub fn line(
        &mut self,
        font: &Font<'_>,
        cache: &mut Glyphs,
        index: u32,
        glyphs: &[u32],
    ) -> Result<(), u32> {
        let number = self.lengths.len();
        // (1 - numlines + line_number) * line_height, in C's int before the double.
        let rows = 1 - self.num_lines as i64 + number as i64;
        let pen_y = (rows * self.line_height) as f64;
        let mut pen_x = 0.0;
        for &id in glyphs {
            let glyph = cache.get(font, id).ok_or(id)?;
            pen_x += glyph.advance;
            // The top only of the first line, the bottom of every line.
            if number == 0 && pen_y - glyph.max_y < self.min_y {
                self.min_y = pen_y - glyph.max_y;
            }
            if pen_y - glyph.min_y > self.max_y {
                self.max_y = pen_y - glyph.min_y;
            }
        }
        // A length is an int there: the fraction is cut.
        self.lengths.push(pen_x as i32);
        if pen_x > self.max_x {
            self.max_x = pen_x;
        }
        if !glyphs.is_empty() {
            self.ids.extend_from_slice(glyphs);
            let line = Line {
                cache: index,
                end: self.ids.len() as u32,
                x: 0.0,
                y: pen_y,
            };
            self.lines.push(line);
        }
        Ok(())
    }

    /// Lays out an empty line, which takes its height but draws nothing.
    pub fn empty_line(&mut self) {
        self.lengths.push(0);
        if 0.0 > self.max_x {
            self.max_x = 0.0;
        }
    }

    /// The text path of the lines laid out, each line of several moved by `align` as
    /// `msLayoutTextSymbol` moves it: the lines are counted as their glyphs change height, so
    /// that a line without glyphs is skipped and those after it take the length of the line
    /// before, as MapServer counts them.
    pub fn finish(self, align: Align) -> TextPath {
        let mut lines = self.lines;
        if self.num_lines > 1 {
            let longest = self.lengths.iter().copied().fold(0, i32::max);
            for (counted, line) in lines.iter_mut().enumerate() {
                let length = self.lengths[counted];
                let offset = match align {
                    Align::Center => f64::from(longest - length) / 2.0,
                    Align::Right => f64::from(longest - length),
                    Align::Left => 0.0,
                };
                line.x += offset;
            }
        }
        TextPath {
            ids: self.ids.into_boxed_slice(),
            lines: lines.into_boxed_slice(),
            min_x: 0.0,
            min_y: self.min_y,
            max_x: self.max_x,
            max_y: self.max_y,
            num_lines: self.num_lines,
        }
    }
}

/// How a glyph's outline is placed: by `msDrawTextSymbol`'s point of the glyph and
/// `agg2RenderGlyphsPath`'s affine transformation, the outline's y flipped, turned by the
/// negated rotation, and moved to the point.
#[derive(Clone, Copy)]
struct Turn {
    /// `cos` and `sin` of the rotation, which turn the glyphs' points about the label's.
    cos: f64,
    sin: f64,
    /// The transformation's `sx`, `shx`, `shy`, `sy`: AGG's rotation by the negated rotation.
    m: [f64; 4],
    turned: bool,
}

impl Turn {
    fn new(rotation: f64) -> Self {
        if rotation == 0.0 {
            return Self {
                cos: 1.0,
                sin: 0.0,
                m: [1.0, 0.0, 0.0, 1.0],
                turned: false,
            };
        }
        let a = -rotation;
        let (sa, ca) = (libm::sin(a), libm::cos(a));
        Self {
            cos: libm::cos(rotation),
            sin: libm::sin(rotation),
            m: [ca, -sa, sa, ca],
            turned: true,
        }
    }

    /// The point of a glyph at `(x, y)` from the label's point `origin`.
    fn glyph_point(&self, x: f64, y: f64, origin: Point) -> Point {
        if self.turned {
            Point {
                x: x * self.cos + y * self.sin + origin.x,
                y: -x * self.sin + y * self.cos + origin.y,
            }
        } else {
            Point {
                x: x + origin.x,
                y: y + origin.y,
            }
        }
    }

    /// A point of a glyph's outline, y up, placed at the glyph's point `at`.
    fn outline_point(&self, p: Point, at: Point) -> Point {
        let (x, y) = (p.x, -p.y);
        if self.turned {
            let [sx, shx, shy, sy] = self.m;
            Point {
                x: x * sx + y * shx + at.x,
                y: x * shy + y * sy + at.y,
            }
        } else {
            Point {
                x: x + at.x,
                y: y + at.y,
            }
        }
    }

    /// The box of a box `[min x, min y, max x, max y]` of outline points, y down, placed at
    /// `at`: of its four corners, turned.
    fn bounds(&self, b: [f64; 4], at: Point) -> [f64; 4] {
        if !self.turned {
            return [at.x + b[0], at.y + b[1], at.x + b[2], at.y + b[3]];
        }
        let mut out = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for (x, y) in [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])] {
            // A corner y down is one of the outline's, y up, at -y.
            let p = self.outline_point(Point { x, y: -y }, at);
            out = [
                out[0].min(p.x),
                out[1].min(p.y),
                out[2].max(p.x),
                out[3].max(p.y),
            ];
        }
        out
    }
}

impl TextPath {
    /// The box the path takes, as [`crate::metrics`] places it.
    pub fn text_box(&self) -> TextBox {
        TextBox {
            min_x: self.min_x,
            min_y: self.min_y,
            max_x: self.max_x,
            max_y: self.max_y,
            num_lines: self.num_lines,
        }
    }

    /// Lays out one line, `glyphs` of `font`, whose glyphs `cache` holds at the label's size and
    /// which [`Self::draw`] finds as the first of its caches. The error names a glyph the font
    /// cannot draw, nor its fallback.
    pub fn new(font: &Font<'_>, cache: &mut Glyphs, glyphs: &[u32]) -> Result<Self, u32> {
        // One line stands on the origin whatever the line height.
        let mut layout = Layout::new(0.0, 1);
        layout.line(font, cache, 0, glyphs)?;
        Ok(layout.finish(Align::Left))
    }

    /// The outlines of the glyphs drawn from `origin`, turned by `rotation` in radians
    /// counter-clockwise, in pixels, out of `caches`, each line's glyphs out of the cache it was
    /// laid out with, as `msDrawTextSymbol` places each glyph and `agg2RenderGlyphsPath` turns
    /// them into one path. Each glyph that lies wholly outside `visible`,
    /// `[min x, min y, max x, max y]`, is left out, so that what a label costs to draw follows what of
    /// it can be seen; a glyph reaching farther than [`MAX_GLYPH_REACH`] from it is left out and
    /// counted.
    pub fn draw(
        &self,
        caches: &[Glyphs],
        origin: Point,
        rotation: f64,
        visible: [f64; 4],
    ) -> Drawn {
        let mut drawn = Drawn::default();
        let turn = Turn::new(rotation);
        let reach = [
            visible[0] - MAX_GLYPH_REACH,
            visible[1] - MAX_GLYPH_REACH,
            visible[2] + MAX_GLYPH_REACH,
            visible[3] + MAX_GLYPH_REACH,
        ];
        let mut start = 0;
        for line in &self.lines {
            let ids = &self.ids[start..line.end as usize];
            start = line.end as usize;
            let Some(cache) = caches.get(line.cache as usize) else {
                continue;
            };
            let mut pen_x = 0.0;
            for id in ids {
                let Some(glyph) = cache.drawn.get(id) else {
                    continue;
                };
                // The glyph's point after the line's alignment, then placed at the label's.
                let at = turn.glyph_point(pen_x + line.x, line.y, origin);
                pen_x += glyph.advance;
                let [x0, y0, x1, y1] =
                    turn.bounds([glyph.min_x, -glyph.max_y, glyph.max_x, -glyph.min_y], at);
                if glyph.path.is_empty()
                    || x1 < visible[0]
                    || x0 > visible[2]
                    || y1 < visible[1]
                    || y0 > visible[3]
                {
                    continue;
                }
                if !(x0 >= reach[0] && y0 >= reach[1] && x1 <= reach[2] && y1 <= reach[3]) {
                    drawn.out_of_reach += 1;
                    continue;
                }
                let place = |p: Point| turn.outline_point(p, at);
                drawn
                    .path
                    .extend(glyph.path.iter().map(|&command| match command {
                        PathCommand::MoveTo(p) => PathCommand::MoveTo(place(p)),
                        PathCommand::LineTo(p) => PathCommand::LineTo(place(p)),
                        PathCommand::QuadTo(c, p) => PathCommand::QuadTo(place(c), place(p)),
                        PathCommand::CubicTo(c0, c1, p) => {
                            PathCommand::CubicTo(place(c0), place(c1), place(p))
                        }
                        PathCommand::Close => PathCommand::Close,
                    }));
            }
        }
        drawn
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
