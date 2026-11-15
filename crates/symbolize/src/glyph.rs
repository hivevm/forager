//! The port a TrueType marker asks its glyph of, which the engine answers through the render's
//! fonts and `label` (ADR-draft-truetype-marker-glyph-through-a-port).

use std::rc::Rc;

use forager_render::PathCommand;

/// A glyph a marker draws: its outline in pixels about its origin, y down as the display list has
/// it, its control box in the same pixels, and whether the font lacks the character, so that
/// glyph 0 stands in for it.
#[derive(Debug, PartialEq)]
pub struct Glyph {
    pub path: Vec<PathCommand>,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub missing: bool,
}

/// The port a TrueType marker asks its glyph of: the glyph of `character` in the font the font
/// set names `font`, at `size` points, or why there is none.
pub trait Glyphs {
    fn glyph(&self, font: &str, character: char, size: u32) -> Result<Rc<Glyph>, String>;
}
