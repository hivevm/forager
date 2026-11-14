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

use crate::hatch::Hatching;
use crate::line_marker::MarkerBudget;
use crate::symbol::{self, Factors, Pixmaps};
use crate::tile::Tiles;

/// What a layer draws its features on beside the view: the map's symbols, the images of its
/// pixmap symbols and the render's tiles of them, the image's width and height in pixels, the
/// layer's scale factor and the image's resolution factor (see the engine's
/// `classes::scales`), what is left to the layer of
/// [`crate::line_marker::MAX_MARKER_POINTS`], the hatching of the class drawn, which counts the
/// hatches refused (Q-9), the pixmap symbols left undrawn, and whether the layer is drawn into an
/// image of its own.
pub struct Canvas<'a> {
    pub(crate) symbols: &'a [Symbol],
    pixmaps: Option<&'a Pixmaps>,
    pub(crate) tiles: Option<&'a mut Tiles>,
    /// The image's width and height in pixels, as the renderer is handed them.
    pub(crate) size: (u32, u32),
    /// The layer's scale factor and the image's resolution factor.
    pub(crate) factors: Factors,
    pub(crate) markers: MarkerBudget,
    pub(crate) hatching: Hatching,
    /// Each pixmap symbol left undrawn, by its index from 1, and why.
    pub(crate) refused_pixmaps: BTreeMap<usize, String>,
    pub(crate) own_image: bool,
}

/// What a layer's drawing left undrawn, which it reports: how many hatches were refused (Q-9),
/// and the pixmap symbols whose image is refused, by their index from 1, each named, with why.
#[derive(Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Refusals {
    pub hatches: usize,
    pub pixmaps: BTreeMap<usize, (String, String)>,
}

impl Refusals {
    /// Adds `other`'s hatches to these, and its pixmap symbols not among these, each by its index
    /// once, the first kept.
    pub fn merge(&mut self, other: Self) {
        self.hatches += other.hatches;
        for (index, named) in other.pixmaps {
            self.pixmaps.entry(index).or_insert(named);
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
            pixmaps: None,
            tiles: None,
            size: (width, height),
            factors,
            markers: MarkerBudget::new(),
            hatching: Hatching::default(),
            refused_pixmaps: BTreeMap::new(),
            own_image: false,
        }
    }

    /// The canvas, its pixmap symbols drawn with the images of `pixmaps`; without them, every
    /// pixmap is left undrawn.
    pub fn with_pixmaps(self, pixmaps: &'a Pixmaps) -> Self {
        Self {
            pixmaps: Some(pixmaps),
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

    /// Marks the layer as drawn into an image of its own, which msDrawLayer creates with
    /// msImageCreate: that image's `refpt` stays at its top left corner, where agg2CreateImage's
    /// `calloc` leaves it, and a hatch counts its lines from there.
    pub fn in_own_image(&mut self) {
        self.own_image = true;
    }

    /// Sets the class whose styles the next features are drawn in, for the clip of its hatches
    /// (see [`Hatching::for_class`]).
    pub fn class(&mut self, styles: &[Style]) {
        self.hatching = Hatching {
            refused: self.hatching.refused,
            ..Hatching::for_class(styles, self.symbols, self.pixmaps, self.factors)
        };
    }

    /// The image's width and height in pixels.
    pub(crate) fn image(&self) -> (f64, f64) {
        (f64::from(self.size.0), f64::from(self.size.1))
    }

    /// Whether markers of the layer were left out, their points past the budget: at its
    /// features, along its lines, or where a style's `GEOMTRANSFORM` places them.
    pub fn markers_spent(&self) -> bool {
        self.markers.spent()
    }

    /// What the layer left undrawn since this was last asked: the hatches refused (Q-9) and the
    /// pixmap symbols whose image is refused, each named by its name or its index (see
    /// [`symbol::named`]).
    pub fn take_refusals(&mut self) -> Refusals {
        let pixmaps = std::mem::take(&mut self.refused_pixmaps)
            .into_iter()
            .map(|(index, why)| (index, (self.named(index), why)))
            .collect();
        Refusals {
            hatches: std::mem::take(&mut self.hatching.refused),
            pixmaps,
        }
    }

    /// The symbol at `index`, from 1, named for a report.
    fn named(&self, index: usize) -> String {
        let symbol = index.checked_sub(1).and_then(|i| self.symbols.get(i));
        symbol::named(index, symbol)
    }

    /// Records the pixmap symbol of `mark`, if it is one, as not drawn where MapServer tiles its
    /// image along a line, without a GAP, which Forager does not: the style's `COLOR` stands in
    /// for it, or, for the first style of a class, its `OUTLINECOLOR` without a `COLOR`; nothing
    /// otherwise. Its image is not read, so that one that does not exist warns too.
    pub(crate) fn untiled_pixmap(&mut self, mark: &symbol::Mark<'_>) {
        let symbol::Mark::Pixmap(index, symbol) = *mark else {
            return;
        };
        self.refused_pixmaps.entry(index).or_insert_with(|| {
            let image = symbol
                .image
                .as_deref()
                .map_or("its image".to_owned(), |path| format!("its image {path}"));
            format!(
                "MapServer tiles {image} along a line without a GAP, which Forager does not \
                 yet; the style's COLOR is drawn in its place, or, for the first style of a \
                 class, its OUTLINECOLOR without a COLOR; nothing otherwise"
            )
        });
    }

    /// The image of the pixmap symbol `symbol`, at `index` from 1, or `None`, its refusal
    /// recorded, where it has none.
    pub(crate) fn pixmap(
        &mut self,
        index: usize,
        symbol: &Symbol,
    ) -> Option<Rc<PremultipliedImage>> {
        let image = match (&symbol.image, self.pixmaps) {
            (None, _) => Err("the pixmap symbol names no image".to_owned()),
            (Some(path), Some(pixmaps)) => match pixmaps.get(path) {
                Some(image) => image.clone(),
                None => Err(format!("the image {path} is not loaded for this render")),
            },
            (Some(path), None) => Err(format!("the image {path} is not loaded for this render")),
        };
        image
            .map_err(|why| {
                self.refused_pixmaps.entry(index).or_insert(why);
            })
            .ok()
    }
}

#[cfg(test)]
mod tests;
