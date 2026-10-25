// The styles follow MapServer's loadStyle, src/mapfile.c, of the release pinned in
// scripts/fetch-msautotest.sh.
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

//! A class's or a label's `STYLE` blocks.

use forager_style_ir::{Bound, GeomTransform, LineCap, LineJoin, LineOffset, Style};

use super::{Range, Translator, kept_binding, kept_bindings};
use crate::lexer::TokenKind;
use crate::syntax::{Block, Directive, Node};

impl Translator<'_> {
    pub(super) fn style(&mut self, block: &Block) -> Style {
        let mut style = Style {
            span: Some(block.span.clone()),
            ..Style::default()
        };
        // SIZE's binding, which a later SIZE's expression hides but does not free.
        let mut size_binding = None;
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "COLOR" => {
                        let color = self.bound_color(d);
                        style.color = kept_binding(style.color.take(), color);
                    }
                    "OUTLINECOLOR" => {
                        let color = self.bound_color(d);
                        style.outline_color = kept_binding(style.outline_color.take(), color);
                    }
                    // The ranges are those loadStyle checks.
                    "WIDTH" => {
                        let width = self.bound_number(d, Range::at_least(0.0), false);
                        style.width = kept_binding(style.width.take(), width);
                    }
                    "SIZE" => style.size = self.size(d, &mut size_binding),
                    "ANGLE" if d.values.len() == 1 && d.values[0].is_word("AUTO") => {
                        style.auto_angle = true;
                    }
                    "ANGLE" => {
                        let range = Range::within(-360.0, 360.0);
                        let angle = self.bound_number(d, range, false);
                        style.angle = kept_binding(style.angle.take(), angle);
                    }
                    "SYMBOL" => {
                        let symbol = self.symbol(d);
                        style.symbol = kept_binding(style.symbol.take(), symbol);
                    }
                    "OFFSET" => self.offset(d, &mut style),
                    "POLAROFFSET" => {
                        let pair = self.bound_pair(d, Range::any());
                        style.polar_offset = kept_bindings(style.polar_offset.take(), pair);
                    }
                    "GAP" => style.gap = self.number(d, Range::any()),
                    "INITIALGAP" => style.initial_gap = self.number(d, Range::at_least(0.0)),
                    "LINECAP" => {
                        style.line_cap = self.choice(
                            d,
                            &[
                                ("BUTT", LineCap::Butt),
                                ("ROUND", LineCap::Round),
                                ("SQUARE", LineCap::Square),
                                ("TRIANGLE", LineCap::Triangle),
                            ],
                        );
                    }
                    "LINEJOIN" => {
                        style.line_join = self.choice(
                            d,
                            &[
                                ("NONE", LineJoin::None),
                                ("ROUND", LineJoin::Round),
                                ("MITER", LineJoin::Miter),
                                ("BEVEL", LineJoin::Bevel),
                            ],
                        );
                    }
                    "LINEJOINMAXSIZE" => {
                        style.line_join_max_size = self.number(d, Range::above(0.0));
                    }
                    // loadStyle truncates an opacity and clamps it to 0..100.
                    "OPACITY" => {
                        let opacity = self.bound_number(d, Range::any(), false).map(|o| match o {
                            Bound::Value(n) => Bound::Value(n.trunc().clamp(0.0, 100.0)),
                            bound => bound,
                        });
                        style.opacity = kept_binding(style.opacity.take(), opacity);
                    }
                    "OUTLINEWIDTH" => {
                        let width = self.bound_number(d, Range::at_least(0.0), false);
                        style.outline_width = kept_binding(style.outline_width.take(), width);
                    }
                    "MINSCALEDENOM" => style.min_scale_denom = self.scale_denom(d),
                    "MAXSCALEDENOM" => style.max_scale_denom = self.scale_denom(d),
                    "MINSIZE" => style.min_size = self.number(d, Range::at_least(0.0)),
                    "MAXSIZE" => style.max_size = self.number(d, Range::above(0.0)),
                    "MINWIDTH" => style.min_width = self.number(d, Range::at_least(0.0)),
                    "MAXWIDTH" => style.max_width = self.number(d, Range::above(0.0)),
                    "GEOMTRANSFORM" => style.geom_transform = self.geom_transform(d),
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) if b.keyword == "PATTERN" => style.pattern = self.pattern(b),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        style
    }

    /// `SIZE`, kept to `binding`, the last binding it had: bindStyle evaluates an expression
    /// after the binding, and loadStyle frees only the expression for a later `SIZE`.
    fn size(&mut self, d: &Directive, binding: &mut Option<Bound<f64>>) -> Option<Bound<f64>> {
        let size = self.bound_number(d, Range::above(0.0), true);
        if matches!(size, Some(Bound::Binding { .. })) {
            binding.clone_from(&size);
        }
        match d.values.as_slice() {
            [token] if matches!(token.kind, TokenKind::Expression(_)) => size,
            _ => kept_binding(binding.clone(), size),
        }
    }

    /// `OFFSET x y`. MapServer writes a line drawn parallel to the feature's as a y of -99, or
    /// -999 for both sides (`MS_STYLE_SINGLE_SIDED_OFFSET`, `MS_STYLE_DOUBLE_SIDED_OFFSET`);
    /// the IR states it as a line offset. Both are the one `offsetx` and `offsety` of loadStyle,
    /// so a later `OFFSET` replaces either, but for the binding of an axis.
    fn offset(&mut self, d: &Directive, style: &mut Style) {
        let Some(pair) = self.bound_pair(d, Range::any()) else {
            return;
        };
        let line_offset = style.line_offset.take().map(|line| {
            let y = if line.both_sides { -999.0 } else { -99.0 };
            [line.distance, Bound::Value(y)]
        });
        let earlier = style.offset.take().or(line_offset);
        let Some([x, y]) = kept_bindings(earlier, Some(pair)) else {
            return;
        };
        match y {
            Bound::Value(n) if n == -99.0 || n == -999.0 => {
                style.line_offset = Some(LineOffset {
                    distance: x,
                    both_sides: n == -999.0,
                });
            }
            y => style.offset = Some([x, y]),
        }
    }

    /// `PATTERN` with 2 to 10 numbers, as loadStyle takes it.
    fn pattern(&mut self, block: &Block) -> Vec<f64> {
        let numbers: Option<Vec<f64>> = block
            .values
            .iter()
            .map(|t| match t.kind {
                TokenKind::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        match numbers {
            Some(numbers) if (2..=10).contains(&numbers.len()) => numbers,
            _ => {
                self.error(
                    "invalid-value",
                    "PATTERN expects 2 to 10 numbers",
                    &block.span,
                );
                Vec::new()
            }
        }
    }

    /// `GEOMTRANSFORM` by name; MapServer's other transforms and its expressions are not yet
    /// carried over.
    fn geom_transform(&mut self, d: &Directive) -> Option<GeomTransform> {
        // An unquoted word is a string to MapServer's lexer.
        let name = match d.values.as_slice() {
            [token] => token
                .string()
                .map(str::to_ascii_lowercase)
                .or_else(|| matches!(token.kind, TokenKind::Expression(_)).then(String::new)),
            _ => None,
        };
        let Some(name) = name else {
            self.invalid(d, "a transform name or an expression");
            return None;
        };
        // msStyleSetGeomTransform, src/mapgeomtransform.c: the first of its names the transform
        // begins with, in its order; `bbox` and `labelcenter` are not carried over.
        let names = [
            ("start", Some(GeomTransform::Start)),
            ("end", Some(GeomTransform::End)),
            ("vertices", Some(GeomTransform::Vertices)),
            ("bbox", None),
            ("labelpnt", Some(GeomTransform::LabelPoint)),
            ("labelpoly", Some(GeomTransform::LabelPoly)),
            ("labelcenter", None),
            ("centroid", Some(GeomTransform::Centroid)),
        ];
        let transform = names
            .into_iter()
            .find(|(prefix, _)| name.starts_with(prefix))
            .and_then(|(_, transform)| transform);
        if transform.is_none() {
            let message = "this GEOMTRANSFORM is not yet supported, and the style is drawn as \
                           if it had none";
            self.warn("unsupported-value", message, &d.span);
        }
        transform
    }
}

#[cfg(test)]
mod tests;
