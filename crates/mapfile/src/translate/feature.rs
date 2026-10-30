// The reading follows MapServer's loadFeature and loadFeaturePoints, src/mapfile.c, and
// msLayerGetItems and msINLINELayerNextShape, src/maplayer.c, of the release pinned in
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

//! A layer's `FEATURE` blocks, its inline features (ADR-0033).

use forager_style_ir::{FeatureProperty, InlineFeature, ShapeType};

use super::Translator;
use crate::lexer::TokenKind;
use crate::syntax::{Block, Directive, Node};
use crate::wkt;

/// The item names of a layer's features: its first `PROCESSING` whose key is `ITEMS`, compared
/// without regard to case and followed by `=` or `:` (`CSLFetchNameValue`), split at `,` as
/// `msStringSplit` splits, a run of commas one separator. MapServer reads them when it draws, so
/// the directive holds wherever it stands.
pub(super) fn item_names(block: &Block) -> Vec<String> {
    let items = block.nodes.iter().find_map(|node| match node {
        Node::Directive(d) if d.keyword == "PROCESSING" => items_value(d),
        _ => None,
    });
    items.map(split_items).unwrap_or_default()
}

/// The value of a `PROCESSING "ITEMS=…"`, or `None` for another key.
pub(super) fn items_value(d: &Directive) -> Option<&str> {
    let [token] = d.values.as_slice() else {
        return None;
    };
    let value = token.string()?;
    let key = value.get(..5)?;
    let rest = &value[5..];
    (key.eq_ignore_ascii_case("ITEMS") && rest.starts_with(['=', ':'])).then(|| &rest[1..])
}

/// `msStringSplit` at `,`: one token more than the runs of commas, an empty one included.
fn split_items(text: &str) -> Vec<String> {
    let mut names = vec![String::new()];
    let mut last = None;
    for c in text.chars() {
        if c == ',' {
            if last != Some(',') {
                names.push(String::new());
            }
        } else if let Some(name) = names.last_mut() {
            name.push(c);
        }
        last = Some(c);
    }
    names
}

/// A feature's `ITEMS` split at `;`, as `msStringSplitComplex` with `MS_ALLOWEMPTYTOKENS` splits
/// them: none of an empty text, an empty value between two separators and after the last.
fn split_values(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split(';').map(str::to_owned).collect()
}

impl Translator<'_> {
    /// A `FEATURE` block as `loadFeature` reads it, of `shape_type`, the type the layer's `TYPE`
    /// before it gives; without one, `loadLayer` refuses it. Its values are named by `names`, `""`
    /// for a name past its values, a value past the names dropped (`msINLINELayerNextShape`).
    pub(super) fn feature(
        &mut self,
        block: &Block,
        shape_type: Option<ShapeType>,
        names: &[String],
    ) -> Option<InlineFeature> {
        let Some(shape_type) = shape_type else {
            let message = "a FEATURE needs the layer's TYPE before it";
            self.error("feature-before-type", message, &block.span);
            return None;
        };
        let mut feature = InlineFeature {
            shape_type,
            parts: Vec::new(),
            properties: Vec::new(),
            text: None,
            span: Some(block.span.clone()),
        };
        let mut values = Vec::new();
        for node in &block.nodes {
            match node {
                // Each block is one part, an empty one included (msAddLine).
                Node::Block(b) if b.keyword == "POINTS" => feature.parts.push(self.pairs(b)?),
                Node::Directive(d) if d.keyword == "ITEMS" => {
                    values = split_values(&self.string(d)?);
                }
                Node::Directive(d) if d.keyword == "TEXT" => feature.text = Some(self.string(d)?),
                Node::Directive(d) if d.keyword == "WKT" => {
                    let text = self.string(d)?;
                    match wkt::read(&text) {
                        // The shape replaces the one read so far, its values and text too.
                        Ok(shape) => {
                            feature.shape_type = shape.shape_type;
                            feature.parts = shape.parts;
                            feature.text = None;
                            values.clear();
                        }
                        Err(reason) => {
                            let message =
                                format!("WKT `{}` cannot be read: {reason}", wkt::excerpt(&text));
                            self.error("invalid-wkt", &message, &d.span);
                            return None;
                        }
                    }
                }
                Node::Directive(d) => self.unhandled_directive(d),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        feature.properties = names
            .iter()
            .enumerate()
            .map(|(i, name)| FeatureProperty {
                name: name.clone(),
                value: values.get(i).cloned().unwrap_or_default(),
            })
            .collect();
        Some(feature)
    }

    /// The `x y` pairs of a feature's `POINTS` block, as `loadFeaturePoints` reads them: numbers
    /// only, in pairs.
    fn pairs(&mut self, block: &Block) -> Option<Vec<[f64; 2]>> {
        let numbers: Option<Vec<f64>> = block
            .values
            .iter()
            .map(|t| match t.kind {
                TokenKind::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        match numbers {
            Some(numbers) if numbers.len() % 2 == 0 => {
                Some(numbers.chunks(2).map(|p| [p[0], p[1]]).collect())
            }
            _ => {
                let message = "POINTS of a FEATURE expects pairs of numbers";
                self.error("invalid-value", message, &block.span);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;
