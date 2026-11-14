// The symbols follow MapServer's loadSymbol and msAddImageSymbol, src/mapsymbol.c, and
// msGetSymbolIndex, classResolveSymbolNames, and resolveSymbolNames, src/mapfile.c, of the release
// pinned in scripts/fetch-msautotest.sh.
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

//! The symbols of a map, its `SYMBOL` blocks and those of its symbol set, and a style's reference
//! to one.

use forager_ports::ResourcePath;
use forager_style_ir::{ANCHOR_CENTRE, Bound, Map, Symbol, SymbolKind, SymbolRef};

use super::{Range, Translator, binding_of};
use crate::lexer::TokenKind;
use crate::syntax::{Block, Directive, Node};

impl Translator<'_> {
    /// A `SYMBOL` block as loadSymbol reads it, its `IMAGE` resolved beside `beside`: the
    /// mapfile for a symbol of the map, the symbol file for one of the set.
    pub(super) fn symbol_block(&mut self, block: &Block, beside: &ResourcePath) -> Option<Symbol> {
        let mut symbol = Symbol {
            name: None,
            kind: SymbolKind::Vector,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: Some(block.span.clone()),
        };
        // A POINTS block appends to the points before it and measures the extent anew from its
        // own: the points from `measured` on.
        let mut points: Vec<[f64; 2]> = Vec::new();
        let mut measured = 0;
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "NAME" => symbol.name = self.string(d),
                    "TYPE" => {
                        symbol.kind = self.choice(
                            d,
                            &[
                                ("VECTOR", SymbolKind::Vector),
                                ("ELLIPSE", SymbolKind::Ellipse),
                                ("PIXMAP", SymbolKind::Pixmap),
                                ("SIMPLE", SymbolKind::Simple),
                                ("TRUETYPE", SymbolKind::Truetype),
                                ("HATCH", SymbolKind::Hatch),
                                ("SVG", SymbolKind::Svg),
                            ],
                        )?;
                    }
                    "FILLED" => {
                        symbol.filled = self.choice(d, &[("TRUE", true), ("FALSE", false)])?;
                    }
                    "ANCHORPOINT" => {
                        let range = Range::within(0.0, 1.0);
                        let n = self.numbers(d, 2)?;
                        if !(range.contains(n[0]) && range.contains(n[1])) {
                            self.invalid(d, "two numbers from 0 to 1");
                            return None;
                        }
                        symbol.anchor_point = Some([n[0], n[1]]);
                    }
                    "CHARACTER" => symbol.character = self.string(d),
                    "FONT" => symbol.font = self.string(d),
                    "IMAGE" => {
                        let value = self.string(d)?;
                        match beside.resolve(&value) {
                            Ok(path) => symbol.image = Some(path.as_str().to_owned()),
                            Err(e) => {
                                let message = format!("IMAGE `{value}` is refused: {e}");
                                self.error("image-refused", &message, &d.span);
                                return None;
                            }
                        }
                    }
                    "TRANSPARENT" => {
                        let n = self.number(d, Range::within(0.0, 255.0))?;
                        symbol.transparent = Some(n as u8);
                    }
                    // loadSymbol ignores it.
                    "ANTIALIAS" => {}
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) if b.keyword == "POINTS" => {
                    let more = self.points(b, points.len())?;
                    measured = points.len();
                    points.extend(more);
                }
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        // The checks at loadSymbol's END.
        let needs_image = matches!(symbol.kind, SymbolKind::Pixmap | SymbolKind::Svg);
        let needs_points = matches!(symbol.kind, SymbolKind::Vector | SymbolKind::Ellipse);
        if needs_image && symbol.image.is_none() {
            let message = "a PIXMAP or SVG symbol needs an IMAGE";
            self.error("invalid-value", message, &block.span);
            return None;
        }
        if needs_points && points.is_empty() {
            let message = "a VECTOR or ELLIPSE symbol needs POINTS";
            self.error("invalid-value", message, &block.span);
            return None;
        }
        if needs_points {
            let anchor = symbol.anchor();
            let vector = symbol.kind == SymbolKind::Vector;
            let (paths, extent) = shape(&points, measured, vector, anchor);
            symbol.paths = paths;
            symbol.extent = Some(extent);
        }
        Some(symbol)
    }

    /// The `[x, y]` pairs of a `POINTS` block, at most MapServer's `MS_MAXVECTORPOINTS` with the
    /// `before` points of the symbol's earlier blocks.
    fn points(&mut self, block: &Block, before: usize) -> Option<Vec<[f64; 2]>> {
        let numbers: Option<Vec<f64>> = block
            .values
            .iter()
            .map(|t| match t.kind {
                TokenKind::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        match numbers {
            Some(numbers) if numbers.len() % 2 == 0 && before + numbers.len() / 2 <= 100 => {
                Some(numbers.chunks(2).map(|p| [p[0], p[1]]).collect())
            }
            _ => {
                let message = "POINTS expects pairs of numbers, at most 100";
                self.error("invalid-value", message, &block.span);
                None
            }
        }
    }

    /// `SYMBOL` by its index, at least 0, by its name, or bound to a property. The name is
    /// resolved against the symbol set when symbols are carried over.
    pub(super) fn symbol(&mut self, d: &Directive) -> Option<Bound<SymbolRef>> {
        if let Some(bound) = binding_of(d) {
            return Some(bound);
        }
        let expected = "a symbol index of at least 0, a symbol name, or an attribute";
        let [token] = d.values.as_slice() else {
            self.invalid(d, expected);
            return None;
        };
        if let TokenKind::Number(n) = token.kind {
            if !(n >= 0.0 && n <= f64::from(u32::MAX)) {
                self.invalid(d, expected);
                return None;
            }
            // Whether the index is past the symbols is known once the styles' images are
            // appended (`image_symbols`).
            let index = n as u32;
            self.numbered.push((index, d.span.clone()));
            return Some(Bound::Value(SymbolRef::Index(index)));
        }
        let Some(value) = token.string() else {
            self.invalid(d, expected);
            return None;
        };
        if let Some(i) = self.named(value) {
            return Some(Bound::Value(SymbolRef::Index(i as u32 + 1)));
        }
        // msAddImageSymbol makes no symbol of an empty name, and classResolveSymbolNames fails
        // the load: "Undefined symbol".
        if value.is_empty() {
            let message = "an empty SYMBOL name names no symbol; MapServer fails the load";
            self.error("invalid-value", message, &d.span);
            return None;
        }
        // msAddImageSymbol fetches a name that begins with `http` with curl.
        if is_url(value) {
            let message = format!(
                "no symbol is named `{value}`; MapServer would fetch it as an image by URL, which \
                 Forager never reads, so its image is not drawn"
            );
            self.warn("symbol-unknown", &message, &d.span);
            return Some(Bound::Value(SymbolRef::Name(value.to_owned())));
        }
        // Any other name is an image file beside the main mapfile, appended to the symbols once
        // the map is read (`image_symbols`); whether it exists is known as it is drawn.
        if let Err(e) = self.main.resolve(value) {
            let message = format!("the image file `{value}` the style names is refused: {e}");
            self.error("image-refused", &message, &d.span);
            return None;
        }
        Some(Bound::Value(SymbolRef::Name(value.to_owned())))
    }

    /// The position among the symbols of the first named `name`, without regard to case, as
    /// msGetSymbolIndex finds it.
    fn named(&self, name: &str) -> Option<usize> {
        self.names.get(&name.to_ascii_lowercase()).copied()
    }

    /// Appends `symbol` to the symbols, and its name, unless an earlier symbol has it.
    pub(super) fn push_symbol(&mut self, symbol: Symbol) {
        if let Some(name) = &symbol.name {
            let position = self.symbols.len();
            self.names
                .entry(name.to_ascii_lowercase())
                .or_insert(position);
        }
        self.symbols.push(symbol);
    }

    /// The image files the styles of `map` name, appended to the symbols as resolveSymbolNames
    /// and classResolveSymbolNames append them: layer by layer and class by class, a class's
    /// styles before its labels' styles, through msGetSymbolIndex, which finds a name without
    /// regard to case from index 1, or else msAddImageSymbol (see [`Self::image_symbol`]). Then
    /// a style's index past the symbols, which msDrawLineSymbol and its kin draw nothing for,
    /// warns.
    pub(super) fn image_symbols(&mut self, map: &mut Map) {
        for class in map.layers.iter_mut().flat_map(|layer| &mut layer.classes) {
            let labels = class.labels.iter_mut().flat_map(|label| &mut label.styles);
            let labels = labels.map(|style| &mut style.symbol);
            for symbol in class.styles.iter_mut().map(|s| &mut s.symbol).chain(labels) {
                let Some(Bound::Value(SymbolRef::Name(name))) = symbol else {
                    continue;
                };
                if let Some(index) = self.image_symbol(name) {
                    *symbol = Some(Bound::Value(SymbolRef::Index(index)));
                }
            }
        }
        let count = self.symbols.len();
        for (index, span) in std::mem::take(&mut self.numbered) {
            if index as usize > count {
                let message = format!("there is no symbol {index}; the style draws nothing");
                self.warn("symbol-unknown", &message, &span);
            }
        }
    }

    /// The index from 1 of the symbol named `name`, appended as msAddImageSymbol appends it
    /// where none is: a symbol named by the file name, an SVG symbol for a `.svg` extension and a
    /// pixmap otherwise, its path against the main mapfile's directory. A name by URL, which
    /// msAddImageSymbol fetches after it has taken its index, takes it too, as a pixmap without
    /// an image, which draws nothing. None for a path the base context refuses.
    fn image_symbol(&mut self, name: &str) -> Option<u32> {
        if let Some(i) = self.named(name) {
            return Some(i as u32 + 1);
        }
        let (kind, image) = if is_url(name) {
            (SymbolKind::Pixmap, None)
        } else {
            let path = self.main.resolve(name).ok()?;
            let svg = name
                .rfind('.')
                .and_then(|dot| name.get(dot..dot + 4))
                .is_some_and(|extension| extension.eq_ignore_ascii_case(".svg"));
            let kind = if svg {
                SymbolKind::Svg
            } else {
                SymbolKind::Pixmap
            };
            (kind, Some(path.as_str().to_owned()))
        };
        self.push_symbol(Symbol {
            name: Some(name.to_owned()),
            kind,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image,
            transparent: None,
            span: None,
        });
        Some(self.symbols.len() as u32)
    }
}

/// Whether msAddImageSymbol fetches `name` by URL: it begins with `http`, without regard to case.
fn is_url(name: &str) -> bool {
    name.get(..4)
        .is_some_and(|start| start.eq_ignore_ascii_case("http"))
}

/// A symbol's points split into lines at MapServer's pen-up `-99 -99`, and its extent, as
/// loadSymbol computes them: the largest x and y, from 0, of the points from `measured` on whose
/// x is not -99, and, for a vector symbol, every point with neither coordinate -99 shifted so
/// that none is negative, or so that the smallest is 0 where the anchor point is not the centre.
fn shape(
    points: &[[f64; 2]],
    measured: usize,
    vector: bool,
    anchor: [f64; 2],
) -> (Vec<Vec<[f64; 2]>>, [f64; 2]) {
    let mut extent = [0.0_f64, 0.0_f64];
    for p in points[measured..].iter().filter(|p| p[0] != -99.0) {
        extent = [extent[0].max(p[0]), extent[1].max(p[1])];
    }
    let drawn = |p: &&[f64; 2]| p[0] != -99.0 && p[1] != -99.0;
    let mut shift = [0.0, 0.0];
    if vector {
        let min_x = points
            .iter()
            .filter(drawn)
            .map(|p| p[0])
            .fold(points[0][0], f64::min);
        let min_y = points
            .iter()
            .filter(drawn)
            .map(|p| p[1])
            .fold(points[0][1], f64::min);
        let shift_positive = anchor != ANCHOR_CENTRE;
        if min_x < 0.0 || min_y < 0.0 || (shift_positive && (min_x != 0.0 || min_y != 0.0)) {
            shift = [min_x, min_y];
            extent = [extent[0] - min_x, extent[1] - min_y];
        }
    }
    let mut paths = vec![Vec::new()];
    for p in points {
        if p[0] == -99.0 && p[1] == -99.0 {
            paths.push(Vec::new());
        } else if let Some(path) = paths.last_mut() {
            // loadSymbol leaves a point with one coordinate -99 where it stands.
            let shift = if drawn(&p) { shift } else { [0.0, 0.0] };
            path.push([p[0] - shift[0], p[1] - shift[1]]);
        }
    }
    paths.retain(|path| !path.is_empty());
    (paths, extent)
}

#[cfg(test)]
mod tests;
