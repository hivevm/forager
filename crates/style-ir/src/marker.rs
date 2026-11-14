//! A symbol's anchor point, and the marker of a label's `labelpoint` style, which `symbolize`
//! computes and `label` bounds (ADR-0008 lets neither depend on the other).

/// The anchor point of a symbol without an `ANCHORPOINT`: its centre, as `msInitSymbol` sets it.
pub const ANCHOR_CENTRE: [f64; 2] = [0.5, 0.5];

/// The shift, before any turn, that puts the anchor point `anchor` of a marker `size` wide and
/// high on the point it is drawn at, as `computeMarkerBounds`, `msAdjustMarkerPos`, and
/// `msImagePolylineMarkers` compute it: `(0.5 - x) * width` across and `(0.5 - y) * height` down.
pub fn anchor_offset([anchor_x, anchor_y]: [f64; 2], (width, height): (f64, f64)) -> (f64, f64) {
    ((0.5 - anchor_x) * width, (0.5 - anchor_y) * height)
}

/// What `computeMarkerBounds` takes of a label's first `labelpoint` style: the width and height
/// `msGetMarkerSize` gives its marker, its symbol's anchor point, its angle in degrees, and its
/// `OFFSET`, scaled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelMarker {
    pub size: (f64, f64),
    pub anchor_point: [f64; 2],
    pub angle: f64,
    pub offset: [f64; 2],
}
