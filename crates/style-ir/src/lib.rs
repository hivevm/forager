//! The style IR: the data model of a map, free of MapServer syntax, and the style document that
//! serializes it as YAML or JSON (ADR-0009, ADR-0014).

mod diagnostic;
mod document;
mod model;

pub use diagnostic::{Diagnostic, Severity, Span};
pub use document::{FORMAT_VERSION, StyleDocument};
pub use model::{Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Style};
