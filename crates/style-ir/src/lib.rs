//! The style IR: the data model of a map, free of MapServer syntax, and the style document that
//! serializes it as YAML or JSON (ADR-0009, ADR-0014); its expressions are a CQL2 tree
//! (ADR-0019).

mod diagnostic;
mod document;
mod expression;
mod model;

pub use diagnostic::{Diagnostic, Severity, Span};
pub use document::{FORMAT_VERSION, StyleDocument};
pub use expression::{Expr, MAX_EXPRESSION_DEPTH, Op};
pub use model::{
    Bound, Class, Extent, GeomTransform, Layer, LayerStatus, LineCap, LineJoin, LineOffset, Map,
    RenderMode, Rgba, SourceRef, Style, Symbol, SymbolKind, SymbolRef, Units,
};
