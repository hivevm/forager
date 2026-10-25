//! The style IR: the data model of a map, free of MapServer syntax, and the style document that
//! serializes it as YAML or JSON (ADR-0009, ADR-0014); its expressions are a CQL2 tree
//! (ADR-0019).

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod diagnostic;
mod document;
mod expression;
mod lenient;
mod migration;
mod model;

pub use diagnostic::{Diagnostic, Severity, Span};
pub use document::{FORMAT_VERSION, Read, StyleDocument};
pub use expression::{Expr, MAX_EXPRESSION_DEPTH, Op};
pub use lenient::MAX_DROPPED_VALUES;
pub use migration::{STEPS, Step};
pub use model::{
    Align, AngleMode, Bound, Class, Extent, Force, GeomTransform, Label, Layer, LayerProjection,
    LayerStatus, LineCap, LineJoin, LineOffset, Map, MinFeatureSize, Position, RenderMode, Rgba,
    SourceRef, Style, Symbol, SymbolKind, SymbolRef, Units,
};
