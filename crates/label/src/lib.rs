//! The text of labels (ADR-0028): a label's sizes, the fonts of its font list in the order
//! MapServer tries them, and its glyphs read with skrifa, laid out and drawn as MapServer lays
//! out and draws a Latin line. It does no I/O: the engine loads the font files and hands their
//! bytes here. It is the one crate that depends on skrifa.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod font;
mod layout;
mod size;

pub use crate::font::{Attempt, Font, Unusable, attempts};
pub use crate::layout::{LineError, TextPath, line};
pub use crate::size::{glyph_size, pixel_size};
