//! The symbolization of a style: a feature's geometry drawn in a style, bound to the feature,
//! turned into the drawing operations of the display list, as MapServer draws it. It is
//! synchronous and does no I/O (ADR-0008); the engine orders what it draws, as MapServer's shape
//! cache orders it, and reports what a layer left undrawn.
//!
//! Its modules import one another in one direction only: `draw` imports `canvas`, `extent`,
//! `geom_transform`, `hatch`, `line`, `marker`, `shade`, and `symbol`; `shade` imports `bounds`,
//! `canvas`, `extent`, `hatch`, `line`, `style`, `symbol`, and `tile`; `marker` imports `bounds`,
//! `canvas`, `line`, `line_marker`, `style`, and `symbol`; `line` imports `bounds`, `canvas`,
//! `line_marker`, `offset`, `style`, and `symbol`; `canvas` imports `hatch`, `line_marker`,
//! `symbol`, and `tile`; `hatch` imports `bounds`, `extent`, `style`, and `symbol`; `line_marker`
//! and `tile` import `bounds` and `symbol`; `symbol` imports `bounds` and `style`;
//! `geom_transform` imports `extent`; `bounds`, `extent`, `offset`, and `style` import none. The
//! test `symbolize_modules.rs` of `crates/arch-test` holds that direction, its tests aside, by a
//! table in this order.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod bounds;
mod canvas;
mod draw;
mod extent;
mod geom_transform;
mod hatch;
mod line;
mod line_marker;
mod marker;
mod offset;
mod shade;
mod style;
mod symbol;
mod tile;

pub use crate::canvas::Canvas;
pub use crate::draw::{can_draw, clip_pixmaps, draw_ops, marked_pixmap, point_markers, vertices};
pub use crate::extent::View;
pub use crate::line::{Pass, outlined};
pub use crate::line_marker::MAX_MARKER_POINTS;
pub use crate::shade::shade;
pub use crate::style::DEG_TO_RAD;
pub use crate::symbol::{
    Mark, Pixmaps, Resolved, mark, marker_size_of, marker_size_with, named_index,
    pixmap_marker_size, resolve, symbol_ops,
};
pub use crate::tile::{MAX_DECODED_IMAGE_BYTES, Tiles};
