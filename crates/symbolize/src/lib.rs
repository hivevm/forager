//! The symbolization of a style: a feature's geometry drawn in a style, bound to the feature,
//! turned into the drawing operations of the display list, as MapServer draws it. It is
//! synchronous and does no I/O (ADR-0009); the engine orders what it draws, as MapServer's shape
//! cache orders it, and reports what a layer left undrawn.
//!
//! Its modules import one another in one direction only: `draw` imports every other; `hatch`
//! imports `bounds`, `extent`, `style`, and `symbol`; `line_marker` imports `bounds` and
//! `symbol`; `symbol` imports `bounds` and `style`; `geom_transform` imports `extent`; `bounds`,
//! `extent`, `offset`, and `style` import none.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod bounds;
mod draw;
mod extent;
mod geom_transform;
mod hatch;
mod line_marker;
mod offset;
mod style;
mod symbol;

pub use crate::draw::{
    Canvas, Pass, Pixmaps, can_draw, draw_ops, marked_pixmap, outlined, shade, vertices,
};
pub use crate::extent::View;
pub use crate::line_marker::MAX_MARKER_POINTS;
pub use crate::style::DEG_TO_RAD;
pub use crate::symbol::{
    Mark, mark, marker_size_of, marker_size_with, pixmap_marker_size, symbol_ops,
};
