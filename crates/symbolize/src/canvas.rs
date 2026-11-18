// The canvas a layer draws on holds what msDrawLineSymbol(), msDrawShadeSymbol(), and
// msDrawMarkerSymbol(), src/maprendering.c of the release pinned in scripts/fetch-msautotest.sh,
// take from the map and the image: the symbols, the images of the pixmaps, the scale factors.
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

use std::collections::BTreeMap;
use std::rc::Rc;

use forager_render::PremultipliedImage;
use forager_style_ir::{Style, Symbol};

use crate::clip::Clip;
use crate::glyph::Glyphs;
use crate::layer_transform::StyleTrees;
use crate::line_marker::MarkerBudget;
use crate::symbol::{self, Factors, Pixmaps, SymbolResources};
use crate::tile::Tiles;

/// What a layer draws its features on beside the view: the map's symbols, the images of its
/// pixmap symbols and the render's tiles of them, the image's width and height in pixels, the
/// layer's scale factor and the image's resolution factor (see the engine's
/// `classes::scales`), what is left to the layer of
/// [`crate::line_marker::MAX_MARKER_POINTS`], the clip of the class drawn, the hatches refused
/// (Q-9), the pixmap symbols left undrawn, what the transform trees of the
/// layer's class styles share, and whether the layer is drawn into an image of its own.
pub struct Canvas<'a> {
    pub(crate) symbols: &'a [Symbol],
    /// The images of the pixmap symbols and the port TrueType markers ask their glyphs of.
    pub(crate) resources: SymbolResources<'a>,
    pub(crate) tiles: Option<&'a mut Tiles>,
    /// The image's width and height in pixels, as the renderer is handed them.
    pub(crate) size: (u32, u32),
    /// The layer's scale factor and the image's resolution factor.
    pub(crate) factors: Factors,
    pub(crate) markers: MarkerBudget,
    pub(crate) clip: Clip,
    /// How many hatches were refused (Q-9), which [`crate::shade::shade`] alone counts.
    pub(crate) refused_hatches: usize,
    /// Each pixmap symbol left undrawn, by its index from 1, and why.
    pub(crate) refused_pixmaps: BTreeMap<usize, String>,
    /// Each vector or TrueType symbol whose fill tile is refused (Q-9), and each TrueType symbol
    /// whose glyph has no font, by its index from 1, and why.
    pub(crate) refused_symbols: BTreeMap<usize, String>,
    /// Each TrueType symbol whose font lacks its character, by its index from 1, and what is
    /// drawn instead.
    pub(crate) missing_glyphs: BTreeMap<usize, String>,
    /// The budget, the `[map_cellsize]`, and the failures of the class styles' transform trees;
    /// without them, a class style's tree draws nothing.
    pub(crate) style_trees: Option<StyleTrees>,
    pub(crate) own_image: bool,
    /// Whether a line shape is clipped before it is drawn, as msDrawShape clips it but for a
    /// layer of `PROCESSING "POLYLINE_NO_CLIP"`.
    pub(crate) clip_lines: bool,
}

/// What a layer's drawing left undrawn, which it reports: whether markers were left out past the
/// budget of markers (Q-9), how many hatches were refused (Q-9), the pixmap symbols whose image is
/// refused, and the vector and TrueType symbols whose fill tile is refused (Q-9), each by its index
/// from 1, named, with why, and the bound of Q-9 the transform trees of its class styles passed, if
/// they did.
#[derive(Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Refusals {
    pub markers_spent: bool,
    pub hatches: usize,
    pub pixmaps: BTreeMap<usize, (String, String)>,
    pub symbols: BTreeMap<usize, (String, String)>,
    /// The TrueType symbols whose font lacks their character, drawn with glyph 0.
    pub glyphs: BTreeMap<usize, (String, String)>,
    /// The bound of Q-9 the transform trees of the class styles passed, once for the layer.
    pub trees_over_limit: Option<String>,
}

impl Refusals {
    /// Adds `other`'s hatches to these, and its symbols not among these, each by its index once,
    /// the first kept; its markers spent and its trees' bound where these have none.
    pub fn merge(&mut self, other: Self) {
        self.markers_spent |= other.markers_spent;
        self.hatches += other.hatches;
        if self.trees_over_limit.is_none() {
            self.trees_over_limit = other.trees_over_limit;
        }
        for (index, named) in other.pixmaps {
            self.pixmaps.entry(index).or_insert(named);
        }
        for (index, named) in other.symbols {
            self.symbols.entry(index).or_insert(named);
        }
        for (index, named) in other.glyphs {
            self.glyphs.entry(index).or_insert(named);
        }
    }
}

impl<'a> Canvas<'a> {
    /// A canvas for one layer at `factors`, the layer's scale factor, which sizes its styles, and
    /// the image's resolution factor, which bounds them (msPrepareImage), its budget of markers
    /// whole.
    pub fn new(symbols: &'a [Symbol], width: u32, height: u32, factors: Factors) -> Self {
        Self {
            symbols,
            resources: SymbolResources::default(),
            tiles: None,
            size: (width, height),
            factors,
            markers: MarkerBudget::new(),
            clip: Clip::default(),
            refused_hatches: 0,
            refused_pixmaps: BTreeMap::new(),
            refused_symbols: BTreeMap::new(),
            missing_glyphs: BTreeMap::new(),
            style_trees: None,
            own_image: false,
            clip_lines: true,
        }
    }

    /// The canvas, the transform trees of its class styles evaluated with `style_trees`, which
    /// carries the layer's budget and `[map_cellsize]`. The engine gives them to the canvas of
    /// every layer it draws; a canvas without them draws no class style's tree.
    pub fn with_style_trees(self, style_trees: StyleTrees) -> Self {
        Self {
            style_trees: Some(style_trees),
            ..self
        }
    }

    /// Why the transform tree of the style last drawn failed on its shape, if it did; it is
    /// forgotten here.
    pub fn take_style_tree_failure(&mut self) -> Option<String> {
        self.style_trees.as_mut().and_then(StyleTrees::take_failure)
    }

    /// The canvas, its pixmap symbols drawn with the images of `pixmaps`; without them, every
    /// pixmap is left undrawn.
    pub fn with_pixmaps(self, pixmaps: &'a Pixmaps) -> Self {
        Self {
            resources: self.resources.with_pixmaps(pixmaps),
            ..self
        }
    }

    /// Sets the factors the next styles are drawn at, as the labels of several layers, each at
    /// its layer's, are drawn on one canvas.
    pub fn set_factors(&mut self, factors: Factors) {
        self.factors = factors;
    }

    /// The canvas, the polygons of its pixmap symbols filled with the tiles of `tiles`; without
    /// them, such a pixmap is left undrawn.
    pub fn with_tiles(self, tiles: &'a mut Tiles) -> Self {
        Self {
            tiles: Some(tiles),
            ..self
        }
    }

    /// The canvas, its TrueType markers drawn with the glyphs of `glyphs`; without them, each is
    /// left undrawn.
    pub fn with_glyphs(self, glyphs: &'a dyn Glyphs) -> Self {
        Self {
            resources: self.resources.with_glyphs(glyphs),
            ..self
        }
    }

    /// The canvas, its pixmap symbols and TrueType markers drawn with `resources`, the same the
    /// engine's label cache sizes a marker with (see [`symbol::marker_size_of`]).
    pub fn with_resources(self, resources: SymbolResources<'a>) -> Self {
        Self { resources, ..self }
    }

    /// The canvas, its line shapes drawn unclipped, as msDrawShape draws those of a line layer of
    /// `PROCESSING "POLYLINE_NO_CLIP"` (`src/mapdraw.c:1341-1344, 2452`).
    pub fn with_lines_unclipped(self) -> Self {
        Self {
            clip_lines: false,
            ..self
        }
    }

    /// Marks the layer as drawn into an image of its own, which msDrawLayer creates with
    /// msImageCreate: that image's `refpt` stays at its top left corner, where agg2CreateImage's
    /// `calloc` leaves it, and a hatch counts its lines from there.
    pub fn in_own_image(&mut self) {
        self.own_image = true;
    }

    /// Sets the class whose styles the next features are drawn in, for the clip of its hatches
    /// and its lines (see [`Clip::for_class`]).
    pub fn class(&mut self, styles: &[Style]) {
        self.clip = Clip::for_class(styles, self.symbols, self.resources.pixmaps, self.factors);
    }

    /// The image's width and height in pixels.
    pub(crate) fn image(&self) -> (f64, f64) {
        (f64::from(self.size.0), f64::from(self.size.1))
    }

    /// Whether markers of the layer were left out, their points past the budget: at its
    /// features, along its lines, or where a style's `GEOMTRANSFORM` places them.
    #[cfg(test)]
    pub(crate) fn markers_spent(&self) -> bool {
        self.markers.spent()
    }

    /// What the layer left undrawn: since this was last asked, the hatches refused (Q-9), the
    /// pixmap symbols whose image is refused, and the vector, ellipse, and TrueType symbols whose
    /// fill tile is refused, each named by its name or its index (see [`symbol::named`]); and
    /// whether its markers were left out past their budget (Q-9) and the bound of Q-9 the
    /// transform trees of its class styles passed, which the layer's budgets hold on to and are
    /// not forgotten here.
    pub fn take_refusals(&mut self) -> Refusals {
        let named = |canvas: &Self, refused: BTreeMap<usize, String>| {
            refused
                .into_iter()
                .map(|(index, why)| (index, (canvas.named(index), why)))
                .collect()
        };
        let refused_pixmaps = std::mem::take(&mut self.refused_pixmaps);
        let refused_symbols = std::mem::take(&mut self.refused_symbols);
        let missing_glyphs = std::mem::take(&mut self.missing_glyphs);
        let pixmaps = named(self, refused_pixmaps);
        let symbols = named(self, refused_symbols);
        let glyphs = named(self, missing_glyphs);
        let trees_over_limit = self
            .style_trees
            .as_ref()
            .and_then(StyleTrees::over_limit)
            .map(str::to_owned);
        Refusals {
            markers_spent: self.markers.spent(),
            hatches: std::mem::take(&mut self.refused_hatches),
            pixmaps,
            symbols,
            glyphs,
            trees_over_limit,
        }
    }

    /// The symbol at `index`, from 1, named for a report.
    fn named(&self, index: usize) -> String {
        let symbol = index.checked_sub(1).and_then(|i| self.symbols.get(i));
        symbol::named(index, symbol)
    }

    /// The render's cache of tiles, or why the layer draws no tile: the engine's canvas always
    /// keeps one; a canvas made without it, as a test's is, refuses the drawing rather than build a
    /// tile that nothing would hold or count (Q-9).
    pub(crate) fn tiles(&mut self) -> Result<&mut Tiles, String> {
        self.tiles
            .as_deref_mut()
            .ok_or_else(|| "no tiles are kept where it is drawn".to_owned())
    }

    /// The image of the pixmap symbol `symbol`, at `index` from 1, or `None`, its refusal
    /// recorded, where it has none.
    pub(crate) fn pixmap(
        &mut self,
        index: usize,
        symbol: &Symbol,
    ) -> Option<Rc<PremultipliedImage>> {
        self.resources
            .pixmap(symbol)
            .map_err(|why| {
                self.refused_pixmaps.entry(index).or_insert(why);
            })
            .ok()
    }
}

#[cfg(test)]
mod tests;
