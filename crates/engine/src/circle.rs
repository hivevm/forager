// The drawing of a circle layer follows MapServer's circleLayerDrawShape() and msDrawShape(),
// src/mapdraw.c, of the release pinned in scripts/fetch-msautotest.sh.
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

//! A layer of the render mode circle, as MapServer's `circleLayerDrawShape` draws it
//! (`src/mapdraw.c:2008-2029`): each feature of the shape of two points `symbolize` makes a
//! circle of (see [`circle`]), its features kept in the layer's projection and only each
//! circle's centre reprojected, and none of its labels drawn.

use forager_ports::{Feature, Geometry};
use forager_render::Draw;
use forager_style_ir::{Diagnostic, Layer};
use forager_symbolize::{Canvas, View, circle, circle_ops};

use crate::classes;
use crate::layer::{Selected, draw_items};
use crate::reproject::{LayerReprojection, points_not_reprojected};

/// Why a circle layer's labels are not drawn: msDrawShape returns after circleLayerDrawShape,
/// which draws no label (`src/mapdraw.c:2390-2396`).
pub(crate) const NO_LABELS: &str = "a circle layer draws no label, as MapServer draws none";

/// The centres of a circle layer's circles: the way they are reprojected into the map's
/// projection, where the layer is reprojected, boxed, as a way is large and most layers draw no
/// circle.
pub(crate) struct Centres(Option<Box<LayerReprojection>>);

impl Centres {
    /// The centres of `layer`'s `features`, left in the layer's projection, reprojected by
    /// `reprojection` as they are drawn; a warning for the circles whose centre does not
    /// reproject. The centres are reprojected here to count them and again as they are drawn,
    /// which keeps the warning with the other warnings of the layer's reprojection.
    pub(crate) fn new(
        reprojection: Option<LayerReprojection>,
        layer: &Layer,
        features: &[Feature],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
        if let Some(reprojection) = &reprojection {
            let failed = features
                .iter()
                .filter_map(|feature| circle(&feature.geometry))
                .filter(|one| reprojection.point(one.centre).is_none())
                .count();
            if failed > 0 {
                diagnostics.push(points_not_reprojected(layer, failed));
            }
        }
        Self(reprojection.map(Box::new))
    }

    /// Draws a circle layer's `selected` features in order, each as its circle, its centre
    /// reprojected where the layer is, and left where it does not reproject, as msProjectPoint
    /// leaves it: every style of its class in its scale range at the map's `scale`, in `view`, as
    /// circleLayerDrawShape draws them. A feature without a circle draws nothing.
    pub(crate) fn draw<R: Draw>(
        &self,
        renderer: &mut R,
        canvas: &mut Canvas,
        selected: &[Selected<'_>],
        (scale, view): (f64, &View),
    ) {
        for (feature, _, styles) in selected {
            let Some(mut circle) = circle(&feature.geometry) else {
                continue;
            };
            if let Some(centre) = self.0.as_ref().and_then(|c| c.point(circle.centre)) {
                circle.centre = centre;
            }
            for style in styles {
                if classes::in_scale(scale, style.min_scale_denom, style.max_scale_denom) {
                    let ops = circle_ops(circle, style, view, canvas);
                    draw_items(renderer, feature.id, ops);
                }
            }
        }
    }
}

/// Whether a circle layer draws nothing of `geometry`, which is no circle; it takes every
/// feature, as msDrawVectorLayer counts each against its `MAXFEATURES`.
pub(crate) fn no_circle(geometry: &Geometry) -> bool {
    circle(geometry).is_none()
}
