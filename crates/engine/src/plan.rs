// The plan of a class follows the order in which MapServer's msDrawVectorLayer(), src/mapdraw.c,
// draws a line layer's styles and their outlines, at once or from its shape cache, of the release
// pinned in scripts/fetch-msautotest.sh.
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

//! The order of the shape cache: which passes of a feature's styles msDrawVectorLayer draws at
//! once, and which the cache draws after every feature, style by style.

use std::borrow::Borrow;

use forager_style_ir::{Bound, RenderMode, Style};
use forager_symbolize::{Pass, outlined};

/// How msDrawVectorLayer draws the features of one class, decided on its styles as written,
/// whatever a feature binds. A line layer caches the shapes of a class of several styles, or of
/// one whose style has an outline width above 0 or bound to a feature's property; an outline
/// width computed by an expression is 0 there, as bindStyle computes none. A cached feature
/// draws its first pass, and its styles with a `GEOMTRANSFORM`, at once, and the rest from the
/// cache, style by style, after every feature. Any other class draws every style of a feature
/// at once.
pub struct Plan {
    mode: RenderMode,
    cached: bool,
    /// Style by style, whether the class binds its `OUTLINEWIDTH` to a feature: the cache then
    /// draws the passes after the first as for an outline, whatever width the feature binds.
    bound_outline: Vec<bool>,
}

/// The passes of a feature: those drawn at once, and those the cache draws after every feature,
/// each with the place of its style in the class.
pub struct Passes<'s> {
    pub at_once: Vec<(&'s Style, Vec<Pass>)>,
    pub cached: Vec<(usize, &'s Style, Vec<Pass>)>,
}

impl Plan {
    /// The plan of a class of a layer of the render mode, from the class's styles as written.
    pub fn new(mode: RenderMode, written: &[Style]) -> Self {
        let cached = mode == RenderMode::Line
            && (written.len() > 1
                || written
                    .first()
                    .is_some_and(|style| match &style.outline_width {
                        Some(Bound::Value(width)) => *width > 0.0,
                        Some(Bound::Binding { .. }) => true,
                        Some(Bound::Expr(_)) | None => false,
                    }));
        let bound_outline = written
            .iter()
            .map(|s| matches!(s.outline_width, Some(Bound::Binding { .. })))
            .collect();
        Self {
            mode,
            cached,
            bound_outline,
        }
    }

    /// Whether msDrawVectorLayer caches the class's shapes.
    pub fn cached(&self) -> bool {
        self.cached
    }

    /// The passes a feature draws in `styles`, the class's styles bound to it. A line layer
    /// draws the first style as its outline when it has an `OUTLINEWIDTH`, then that first style
    /// itself, then each further style with its outline before it. lineLayerDrawShape draws
    /// every later style with a `GEOMTRANSFORM` in the first pass, as written, and the cache's
    /// passes skip every style with one.
    pub fn passes<'s, S: Borrow<Style>>(&self, styles: &'s [S]) -> Passes<'s> {
        debug_assert_eq!(styles.len(), self.bound_outline.len());
        if self.mode != RenderMode::Line {
            return Passes {
                at_once: styles
                    .iter()
                    .map(|s| (s.borrow(), vec![Pass::Plain]))
                    .collect(),
                cached: Vec::new(),
            };
        }
        let mut passes = Vec::new();
        for (i, style) in styles.iter().map(Borrow::borrow).enumerate() {
            let drawn_outlined = outlined(style);
            let outlined = drawn_outlined || self.bound_outline.get(i).copied().unwrap_or(false);
            if i == 0 {
                let first = if drawn_outlined {
                    Pass::Outline
                } else {
                    Pass::Plain
                };
                passes.push((i, style, vec![first]));
                if outlined {
                    passes.push((i, style, vec![Pass::PlainIfColored]));
                }
            } else if outlined {
                passes.push((
                    i,
                    style,
                    vec![Pass::OutlineIfOutlined, Pass::PlainIfColored],
                ));
            } else {
                passes.push((i, style, vec![Pass::PlainIfColored]));
            }
        }
        if !self.cached {
            return Passes {
                at_once: passes.into_iter().map(|(_, s, p)| (s, p)).collect(),
                cached: Vec::new(),
            };
        }
        let mut passes = passes.into_iter();
        let (_, first_style, first) = passes.next().expect("a cached class has a style");
        let mut at_once = vec![(first_style, first)];
        let mut cached = Vec::new();
        for (place, style, kinds) in passes {
            if style.geom_transform.is_none() {
                cached.push((place, style, kinds));
            } else if place != 0 {
                at_once.push((style, vec![Pass::Plain]));
            }
        }
        Passes { at_once, cached }
    }
}

#[cfg(test)]
mod tests {
    use forager_style_ir::{GeomTransform, Rgba};

    use super::*;

    const BLUE: Rgba = Rgba::opaque(30, 60, 200);
    const BLACK: Rgba = Rgba::opaque(0, 0, 0);

    fn colored(color: Option<Rgba>, outline: Option<Rgba>) -> Style {
        Style {
            color: color.map(Bound::Value),
            outline_color: outline.map(Bound::Value),
            ..Style::default()
        }
    }

    /// The passes drawn at once, and those the cache draws, each with its style's place.
    type Kinds = (Vec<Vec<Pass>>, Vec<(usize, Vec<Pass>)>);

    fn kinds(plan: &Plan, styles: &[Style]) -> Kinds {
        let passes = plan.passes(styles);
        (
            passes.at_once.into_iter().map(|(_, p)| p).collect(),
            passes.cached.into_iter().map(|(i, _, p)| (i, p)).collect(),
        )
    }

    #[test]
    fn a_line_layer_draws_outlines_in_passes_before_the_lines() {
        let outlined = Style {
            outline_width: Some(Bound::Value(1.0)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let plain = colored(Some(BLUE), None);
        let styles = [outlined.clone(), plain.clone(), outlined];
        let plan = Plan::new(RenderMode::Line, &styles);
        assert!(plan.cached());
        assert_eq!(
            kinds(&plan, &styles),
            (
                vec![vec![Pass::Outline]],
                vec![
                    (0, vec![Pass::PlainIfColored]),
                    (1, vec![Pass::PlainIfColored]),
                    (2, vec![Pass::OutlineIfOutlined, Pass::PlainIfColored]),
                ]
            )
        );
        let plain_first = [plain.clone(), plain.clone()];
        let plan = Plan::new(RenderMode::Line, &plain_first);
        assert_eq!(
            kinds(&plan, &plain_first),
            (
                vec![vec![Pass::Plain]],
                vec![(1, vec![Pass::PlainIfColored])]
            )
        );
        // One style without an outline is not cached and draws once.
        let one = [plain];
        let plan = Plan::new(RenderMode::Line, &one);
        assert!(!plan.cached());
        assert_eq!(kinds(&plan, &one), (vec![vec![Pass::Plain]], vec![]));
    }

    #[test]
    fn a_bound_outline_width_caches_the_class_and_draws_its_later_pass_as_for_an_outline() {
        // The written style binds OUTLINEWIDTH; the feature binds 0, so its first pass is plain.
        let written = [Style {
            outline_width: Some(Bound::Binding {
                binding: "w".into(),
            }),
            ..colored(Some(BLUE), None)
        }];
        let bound = [Style {
            outline_width: Some(Bound::Value(0.0)),
            ..colored(Some(BLUE), None)
        }];
        let plan = Plan::new(RenderMode::Line, &written);
        assert!(plan.cached());
        assert_eq!(
            kinds(&plan, &bound),
            (
                vec![vec![Pass::Plain]],
                vec![(0, vec![Pass::PlainIfColored])]
            )
        );
    }

    #[test]
    fn a_style_with_a_geomtransform_draws_at_once_and_the_cache_skips_it() {
        let start = Style {
            geom_transform: Some(GeomTransform::Start),
            outline_width: Some(Bound::Value(2.0)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let end = Style {
            geom_transform: Some(GeomTransform::End),
            ..colored(Some(BLUE), None)
        };
        let plain = colored(Some(BLUE), None);
        let styles = [start, plain, end];
        let plan = Plan::new(RenderMode::Line, &styles);
        // The first style draws once as its outline, the later transformed one as written.
        assert_eq!(
            kinds(&plan, &styles),
            (
                vec![vec![Pass::Outline], vec![Pass::Plain]],
                vec![(1, vec![Pass::PlainIfColored])]
            )
        );
    }

    #[test]
    fn a_layer_other_than_a_line_layer_draws_every_style_at_once() {
        let styles = [colored(Some(BLUE), None), colored(Some(BLACK), None)];
        let plan = Plan::new(RenderMode::Polygon, &styles);
        assert!(!plan.cached());
        assert_eq!(
            kinds(&plan, &styles),
            (vec![vec![Pass::Plain], vec![Pass::Plain]], vec![])
        );
    }
}
