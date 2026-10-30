// The labels follow MapServer's loadLabel, src/mapfile.c, and src/mapserver.h, of the release
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

//! A class's `LABEL` blocks and the `TEXT` they draw.

use forager_style_ir::{
    Align, AngleMode, Bound, Expr, Force, GeomTransform, Label, MinFeatureSize, Position,
};

use super::{Range, Translator, binding_of, kept_binding, kept_bindings};
use crate::expression;
use crate::lexer::TokenKind;
use crate::syntax::{Block, Directive, Node};

impl Translator<'_> {
    /// A label's or class's `TEXT`. One that cannot be carried over is a warning and an empty
    /// text, which draws no label, as MapServer draws none for a text it cannot evaluate.
    pub(super) fn text(&mut self, d: &Directive) -> Option<Expr> {
        let [token] = d.values.as_slice() else {
            self.invalid(d, "one string or expression");
            return None;
        };
        match expression::text(token) {
            Ok(expr) => Some(expr),
            Err(problem) if problem.error => {
                self.error(problem.code, &problem.message, &d.span);
                None
            }
            Err(problem) => {
                let message = format!("{}; the label has no text", problem.message);
                self.warn(problem.code, &message, &d.span);
                Some(Expr::String(String::new()))
            }
        }
    }

    /// The errors of a `TEXT` whose text is not kept, as a later `TEXT` replaces it.
    pub(super) fn text_errors(&mut self, d: &Directive) {
        let [token] = d.values.as_slice() else {
            self.invalid(d, "one string or expression");
            return;
        };
        if let Err(problem) = expression::text(token)
            && problem.error
        {
            self.error(problem.code, &problem.message, &d.span);
        }
    }

    /// A `LABEL` as loadLabel reads it, its text falling back on `fallback`.
    pub(super) fn label(
        &mut self,
        block: &Block,
        fallback: Option<&Expr>,
        label_item: Option<&str>,
    ) -> Label {
        let mut label = Label {
            span: Some(block.span.clone()),
            ..Label::default()
        };
        let mut text = None;
        // PRIORITY's binding, which a later PRIORITY's expression hides but does not free.
        let mut priority_binding = None;
        for node in &block.nodes {
            match node {
                Node::Directive(d) if self.bound_property(d, &mut label, &mut priority_binding) => {
                }
                Node::Directive(d) => match d.keyword.as_str() {
                    "TEXT" => text = self.text(d),
                    // msGetLabelStatus, src/maputil.c, evaluates it against the layer's
                    // LABELITEM; a label whose expression does not match is not drawn.
                    "EXPRESSION" => label.expression = self.expression(d, label_item),
                    "MINSCALEDENOM" => label.min_scale_denom = self.scale_denom(d),
                    "MAXSCALEDENOM" => label.max_scale_denom = self.scale_denom(d),
                    "MINSIZE" => label.min_size = self.number(d, Range::above(0.0)).map(f64::trunc),
                    "MAXSIZE" => label.max_size = self.number(d, Range::above(0.0)).map(f64::trunc),
                    // loadLabel reads it with loadColor(…, NULL), which takes no binding.
                    "SHADOWCOLOR" if binding_of::<()>(d).is_some() => {
                        let message = "SHADOWCOLOR takes a colour, not an attribute";
                        self.error("invalid-value", message, &d.span);
                    }
                    "SHADOWCOLOR" => label.shadow_color = self.color(d).map(Bound::Value),
                    "OUTLINEWIDTH" => {
                        label.outline_width = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "WRAP" => {
                        label.wrap = self
                            .string(d)
                            .and_then(|w| w.chars().next())
                            .map(String::from);
                    }
                    "MAXLENGTH" => {
                        label.max_length = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "BUFFER" => label.buffer = self.number(d, Range::any()).map(f64::trunc),
                    "MINDISTANCE" => {
                        label.min_distance = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "REPEATDISTANCE" => {
                        label.repeat_distance = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "MINFEATURESIZE" if d.values.len() == 1 && d.values[0].is_word("AUTO") => {
                        label.min_feature_size = Some(MinFeatureSize::Auto);
                    }
                    "MINFEATURESIZE" => {
                        label.min_feature_size = self
                            .number(d, Range::above(0.0))
                            .map(|n| MinFeatureSize::Pixels(n.trunc()));
                    }
                    "MAXOVERLAPANGLE" => {
                        label.max_overlap_angle = self.number(d, Range::within(0.0, 360.0));
                    }
                    "PARTIALS" => {
                        label.partials = self.choice(d, &[("TRUE", true), ("FALSE", false)]);
                    }
                    "FORCE" => {
                        label.force = self.choice(
                            d,
                            &[
                                ("ON", Force::On),
                                ("OFF", Force::Off),
                                ("GROUP", Force::Group),
                            ],
                        );
                    }
                    // loadLabel reads and discards them, TYPE as one of its two keywords.
                    "TYPE" => {
                        self.choice(d, &[("TRUETYPE", ()), ("BITMAP", ())]);
                    }
                    "ANTIALIAS" => {}
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) if b.keyword == "STYLE" => {
                    let mut style = self.style(b);
                    // loadLabel's default for a label's style.
                    style
                        .geom_transform
                        .get_or_insert(GeomTransform::LabelPoint);
                    label.styles.push(style);
                }
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        label.text = text.or_else(|| fallback.cloned());
        label
    }

    /// A property that may take a binding, `true` once `d` is one; `priority_binding` is the binding
    /// [`Self::priority`] keeps.
    fn bound_property(
        &mut self,
        d: &Directive,
        label: &mut Label,
        priority_binding: &mut Option<Bound<f64>>,
    ) -> bool {
        match d.keyword.as_str() {
            "FONT" => {
                let font = self.bound_string(d);
                if let Some(Bound::Value(names)) = &font {
                    self.check_fonts(names, &d.span);
                }
                label.font = kept_binding(label.font.take(), font);
            }
            "SIZE" => label.size = self.label_size(d),
            "COLOR" => {
                let color = self.bound_color(d);
                label.color = kept_binding(label.color.take(), color);
            }
            "OUTLINECOLOR" => {
                let color = self.bound_color(d);
                label.outline_color = kept_binding(label.outline_color.take(), color);
            }
            "SHADOWSIZE" => {
                let size = self.integer_pair(d);
                label.shadow_size = kept_bindings(label.shadow_size.take(), size);
            }
            "POSITION" => {
                let position = self.position(d);
                label.position = kept_binding(label.position.take(), position);
            }
            "OFFSET" => {
                let offset = self.integer_pair(d);
                label.offset = kept_bindings(label.offset.take(), offset);
            }
            "ANGLE" => self.label_angle(d, label),
            "ALIGN" => {
                let align = self.bound_choice(
                    d,
                    &[
                        ("LEFT", Align::Left),
                        ("CENTER", Align::Center),
                        ("RIGHT", Align::Right),
                    ],
                );
                label.align = kept_binding(label.align.take(), align);
            }
            "PRIORITY" => label.priority = self.priority(d, priority_binding),
            _ => return false,
        }
        true
    }

    /// A label's `SIZE`: a number above 0, MapServer's bitmap sizes, an attribute, or an
    /// expression.
    fn label_size(&mut self, d: &Directive) -> Option<Bound<f64>> {
        // MS_TINY to MS_GIANT, src/mapserver.h.
        let bitmap = [
            ("TINY", 5.0),
            ("SMALL", 7.0),
            ("MEDIUM", 10.0),
            ("LARGE", 13.0),
            ("GIANT", 16.0),
        ];
        if let [token] = d.values.as_slice()
            && let Some(word) = token.keyword()
            && let Some((_, size)) = bitmap.iter().find(|(name, _)| *name == word)
        {
            return Some(Bound::Value(*size));
        }
        self.bound_number(d, Range::above(0.0), true)
    }

    /// A label's `ANGLE`: a number from -360 to 360, an attribute, or `AUTO`, `AUTO2`, `FOLLOW`.
    fn label_angle(&mut self, d: &Directive, label: &mut Label) {
        let modes = [
            ("AUTO", AngleMode::Auto),
            ("AUTO2", AngleMode::Auto2),
            ("FOLLOW", AngleMode::Follow),
        ];
        if let [token] = d.values.as_slice()
            && let Some(word) = token.keyword()
            && let Some((_, mode)) = modes.iter().find(|(name, _)| *name == word)
        {
            label.angle_mode = Some(*mode);
            return;
        }
        let angle = self.bound_number(d, Range::within(-360.0, 360.0), false);
        label.angle = kept_binding(label.angle.take(), angle);
    }

    /// `PRIORITY`, kept to `binding`, the last binding it had: loadLabel frees only the
    /// expression for a later `PRIORITY`, and bindLabel, src/maputil.c, evaluates an expression
    /// after the binding. A number is cast to an integer, `(int)msyynumber`.
    fn priority(&mut self, d: &Directive, binding: &mut Option<Bound<f64>>) -> Option<Bound<f64>> {
        let priority = self
            .bound_number(d, Range::within(1.0, 10.0), true)
            .map(truncated);
        if matches!(priority, Some(Bound::Binding { .. })) {
            binding.clone_from(&priority);
        }
        match d.values.as_slice() {
            [token] if matches!(token.kind, TokenKind::Expression(_)) => priority,
            _ => kept_binding(binding.clone(), priority),
        }
    }

    /// `OFFSET` or `SHADOWSIZE`: two numbers or attributes, a number cast to an integer,
    /// `(int)msyynumber`, as loadLabel stores it.
    fn integer_pair(&mut self, d: &Directive) -> Option<[Bound<f64>; 2]> {
        self.bound_pair(d, Range::any())
            .map(|pair| pair.map(truncated))
    }

    fn position(&mut self, d: &Directive) -> Option<Bound<Position>> {
        self.bound_choice(
            d,
            &[
                ("UL", Position::Ul),
                ("UC", Position::Uc),
                ("UR", Position::Ur),
                ("CL", Position::Cl),
                ("CC", Position::Cc),
                ("CR", Position::Cr),
                ("LL", Position::Ll),
                ("LC", Position::Lc),
                ("LR", Position::Lr),
                ("AUTO", Position::Auto),
            ],
        )
    }
}

/// A number truncated toward zero, as C's cast to `int` stores it.
fn truncated(bound: Bound<f64>) -> Bound<f64> {
    match bound {
        Bound::Value(n) => Bound::Value(n.trunc()),
        bound => bound,
    }
}

#[cfg(test)]
mod tests;
