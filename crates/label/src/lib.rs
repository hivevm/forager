//! The text of labels (ADR-0028): a label's sizes, the fonts of its font list in the order
//! MapServer tries them, its glyphs read with skrifa, laid out and drawn as MapServer lays out
//! and draws Latin lines, and where a label stands about its point. It does no I/O: the engine
//! loads the font files and hands their bytes here. It is the one crate that depends on skrifa.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod font;
mod layout;
mod place;
mod size;

pub use crate::font::{Attempt, Font, Unusable, attempts};
pub use crate::layout::{Align, Drawn, Glyphs, Layout, LineError, TextPath, lines};
pub use crate::place::{
    AUTO_POSITIONS, Anchor, Bounds, Offsets, Placement, PointLabel, Position, TextBox, metrics,
    place,
};
pub use crate::size::{glyph_size, pixel_size};
