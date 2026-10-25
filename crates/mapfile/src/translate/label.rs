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

use super::{Range, Translator};
use crate::expression;
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

    /// A `LABEL` as loadLabel reads it, its text falling back on `fallback`.
    pub(super) fn label(&mut self, block: &Block, fallback: Option<&Expr>) -> Label {
        let mut label = Label {
            span: Some(block.span.clone()),
            ..Label::default()
        };
        let mut text = None;
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "TEXT" => text = self.text(d),
                    "FONT" => {
                        label.font = self.bound_string(d);
                        if let Some(Bound::Value(names)) = &label.font {
                            self.check_fonts(names, &d.span);
                        }
                    }
                    "SIZE" => label.size = self.label_size(d),
                    "MINSCALEDENOM" => label.min_scale_denom = self.scale_denom(d),
                    "MAXSCALEDENOM" => label.max_scale_denom = self.scale_denom(d),
                    "MINSIZE" => label.min_size = self.number(d, Range::above(0.0)).map(f64::trunc),
                    "MAXSIZE" => label.max_size = self.number(d, Range::above(0.0)).map(f64::trunc),
                    "COLOR" => label.color = self.bound_color(d),
                    "OUTLINECOLOR" => label.outline_color = self.bound_color(d),
                    "OUTLINEWIDTH" => {
                        label.outline_width = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "SHADOWCOLOR" => label.shadow_color = self.bound_color(d),
                    "SHADOWSIZE" => label.shadow_size = self.bound_pair(d, Range::any()),
                    "POSITION" => label.position = self.position(d),
                    "OFFSET" => label.offset = self.bound_pair(d, Range::any()),
                    "ANGLE" => self.label_angle(d, &mut label),
                    "ALIGN" => {
                        label.align = self.bound_choice(
                            d,
                            &[
                                ("LEFT", Align::Left),
                                ("CENTER", Align::Center),
                                ("RIGHT", Align::Right),
                            ],
                        );
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
                    "PRIORITY" => {
                        label.priority = self.bound_number(d, Range::within(1.0, 10.0), true);
                    }
                    // loadLabel reads and discards them.
                    "TYPE" | "ANTIALIAS" => {}
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
        label.angle = self.bound_number(d, Range::within(-360.0, 360.0), false);
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

#[cfg(test)]
mod tests;
