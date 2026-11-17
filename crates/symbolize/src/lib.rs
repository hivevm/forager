//! The symbolization of a style: a feature's geometry drawn in a style, bound to the feature,
//! turned into the drawing operations of the display list, as MapServer draws it. It is
//! synchronous and does no I/O (ADR-0008); the engine orders what it draws, as MapServer's shape
//! cache orders it, and reports what a layer left undrawn.
//!
//! Its modules import one another in one direction only: `draw` imports `canvas`, `clip`,
//! `extent`, `geom_transform`, `layer_transform`, `line`, `marker`, `shade`, and `symbol`; `label`
//! imports `canvas`, `extent`, `marker`, `shade`, and `symbol`; `circle` imports `canvas`,
//! `extent`, `shade`, and `symbol`; `shade` imports `bounds`, `canvas`, `extent`, `hatch`, `line`,
//! `mark`, `style`, `symbol`, and `tile`; `marker` imports `bounds`, `canvas`, `glyph_marker`,
//! `line`, `line_marker`, `style`, and `symbol`; `line` imports `bounds`, `canvas`, `mark`,
//! `offset`, `style`, `symbol`, and `tile`; `mark` imports `canvas`, `glyph`, `glyph_marker`,
//! `line_marker`, `symbol`, and `tile`; `glyph_marker` imports `canvas`, `glyph`, `style`, and
//! `symbol`; `canvas` imports `clip`, `glyph`, `hatch`, `layer_transform`, `line_marker`,
//! `symbol`, and `tile`; `clip` and `hatch` import `bounds`, `extent`, `style`, and `symbol`;
//! `line_marker` and `tile` import `bounds` and `symbol`; `symbol` imports `bounds`, `glyph`, and
//! `style`; `geom_transform` imports `extent`; `layer_transform` imports `geometry`; `bounds`,
//! `extent`, `geometry`, `glyph`, `offset`, and `style` import none. The test
//! `symbolize_modules.rs` of `crates/arch-test` holds that direction, its tests aside, by a table
//! in this order.

// The crate's own clippy.toml adds bans to Q-2's; its header says which.
#![warn(clippy::disallowed_types, clippy::disallowed_methods)]
// C's `(int)` cast is `forager_style_ir::c::int_cast`, never `as` (ADR-0017).
#![deny(clippy::cast_possible_truncation)]

mod bounds;
mod canvas;
mod circle;
mod clip;
mod draw;
mod extent;
mod geom_transform;
mod geometry;
mod glyph;
mod glyph_marker;
mod hatch;
mod label;
mod layer_transform;
mod line;
mod line_marker;
mod mark;
mod marker;
mod offset;
mod shade;
mod style;
mod symbol;
mod tile;

pub use crate::canvas::{Canvas, Refusals};
pub use crate::circle::{Circle, circle, circle_ops};
pub use crate::draw::{can_draw, clip_pixmaps, draw_ops, marked_pixmap, point_markers, vertices};
pub use crate::extent::View;
pub use crate::glyph::{Glyph, Glyphs};
pub use crate::label::{
    LabelGeometry, MarkerValues, label_pixmap, label_style_marker, label_style_ops,
    label_style_refused,
};
pub use crate::layer_transform::{
    Budget, MAX_ADDED_POINTS, MAX_TRANSFORM_VISITS, StyleTrees, Undrawn, transformed,
};
pub use crate::line::{Pass, outlined};
pub use crate::line_marker::MAX_MARKER_POINTS;
pub use crate::shade::shade;
pub use crate::symbol::{
    Factors, Mark, Pixmaps, Resolved, mark, marker_size_of, marker_size_with, pixmap_marker_size,
    resolve, symbol_ops,
};
pub use crate::tile::{MAX_DECODED_IMAGE_BYTES, Tiles};
