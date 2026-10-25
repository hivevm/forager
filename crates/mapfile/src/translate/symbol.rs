// The symbols follow MapServer's loadSymbol, src/mapsymbol.c, and msGetSymbolIndex,
// src/mapfile.c, of the release pinned in scripts/fetch-msautotest.sh.
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
use forager_style_ir::{Bound, Symbol, SymbolKind, SymbolRef};

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
        let mut points: Option<Vec<[f64; 2]>> = None;
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
                Node::Block(b) if b.keyword == "POINTS" => points = Some(self.points(b)?),
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
        let points = points.unwrap_or_default();
        if needs_points && points.is_empty() {
            let message = "a VECTOR or ELLIPSE symbol needs POINTS";
            self.error("invalid-value", message, &block.span);
            return None;
        }
        if needs_points {
            let anchor = symbol.anchor_point.unwrap_or([0.5, 0.5]);
            let (paths, extent) = shape(&points, symbol.kind == SymbolKind::Vector, anchor);
            symbol.paths = paths;
            symbol.extent = Some(extent);
        }
        Some(symbol)
    }

    /// The `[x, y]` pairs of a `POINTS` block, at most MapServer's `MS_MAXVECTORPOINTS`.
    fn points(&mut self, block: &Block) -> Option<Vec<[f64; 2]>> {
        let numbers: Option<Vec<f64>> = block
            .values
            .iter()
            .map(|t| match t.kind {
                TokenKind::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        match numbers {
            Some(numbers) if numbers.len() % 2 == 0 && numbers.len() <= 200 => {
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
            let index = n as u32;
            if index as usize > self.symbols.len() {
                // msDrawLineSymbol and its kin draw nothing for an index past the set.
                let message = format!("there is no symbol {index}; the style draws nothing");
                self.warn("symbol-unknown", &message, &d.span);
            }
            return Some(Bound::Value(SymbolRef::Index(index)));
        }
        let Some(value) = token.string() else {
            self.invalid(d, expected);
            return None;
        };
        // msGetSymbolIndex: by name, without regard to case, the first from index 1.
        let found = self.symbols.iter().position(|symbol| {
            symbol
                .name
                .as_ref()
                .is_some_and(|name| name.eq_ignore_ascii_case(value))
        });
        match found {
            Some(i) => Some(Bound::Value(SymbolRef::Index(i as u32 + 1))),
            None => {
                let message = format!(
                    "no symbol is named `{value}`; MapServer would read it as an image file, \
                     which is not yet supported"
                );
                self.warn("symbol-unknown", &message, &d.span);
                Some(Bound::Value(SymbolRef::Name(value.to_owned())))
            }
        }
    }
}

/// A symbol's points split into lines at MapServer's pen-up `-99 -99`, and its extent, as
/// loadSymbol computes them: the largest x and y of the points whose x is not -99, and, for a
/// vector symbol, every point shifted so that none is negative, or so that the smallest is 0
/// where the anchor point is not the centre.
fn shape(points: &[[f64; 2]], vector: bool, anchor: [f64; 2]) -> (Vec<Vec<[f64; 2]>>, [f64; 2]) {
    let mut extent = [0.0_f64, 0.0_f64];
    for p in points.iter().filter(|p| p[0] != -99.0) {
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
        let shift_positive = anchor != [0.5, 0.5];
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
            path.push([p[0] - shift[0], p[1] - shift[1]]);
        }
    }
    paths.retain(|path| !path.is_empty());
    (paths, extent)
}

#[cfg(test)]
mod tests;
