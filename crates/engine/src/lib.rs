//! The engine: renders the map of a style IR from the sources bound to its layers. It is the
//! one core crate that awaits the ports (ADR-0009).

mod extent;

use std::collections::BTreeMap;

use forager_ports::{Coord, Feature, FeatureQuery, FeatureSource, Geometry};
use forager_render::{DisplayItem, DrawOp, Point, Renderer, RgbaImage};
use forager_style_ir::{Diagnostic, Layer, LayerStatus, Map, RenderMode, Rgba, Style};

pub use crate::extent::View;

/// The largest width or height of an image, MapServer's default `MAXSIZE` (Q-9).
pub const MAX_IMAGE_SIDE: u32 = 4096;

/// MapServer's default `IMAGECOLOR`.
const DEFAULT_BACKGROUND: Rgba = Rgba::opaque(255, 255, 255);

/// The result of a render: the image unless the map could not be drawn at all, and the
/// diagnostics, among them one for every layer skipped.
#[derive(Debug)]
pub struct Rendered {
    pub image: Option<RgbaImage>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Renders `map` with `renderer`, reading each layer's features from the source bound to the
/// layer's name in `sources`. A layer without a source, or whose source fails, is skipped with a
/// diagnostic (G-8).
pub async fn render<S: FeatureSource, R: Renderer>(
    map: &Map,
    sources: &BTreeMap<String, S>,
    mut renderer: R,
) -> Rendered {
    let mut diagnostics = Vec::new();
    let Some((width, height, view)) = view(map, &mut diagnostics) else {
        return Rendered {
            image: None,
            diagnostics,
        };
    };
    renderer.begin(width, height, map.image_color.unwrap_or(DEFAULT_BACKGROUND));
    for layer in &map.layers {
        if layer.status == Some(LayerStatus::Off) {
            continue;
        }
        let Some(mode) = drawable_mode(layer, &mut diagnostics) else {
            continue;
        };
        let Some(features) = features(layer, sources, &view, &mut diagnostics).await else {
            continue;
        };
        // Classes are not yet selected by expression, so the first class draws every feature.
        let Some(class) = layer.classes.first() else {
            continue;
        };
        for feature in &features {
            for style in &class.styles {
                for op in draw_ops(mode, &feature.geometry, style, &view) {
                    renderer.draw(&DisplayItem {
                        op,
                        feature_id: Some(feature.id),
                    });
                }
            }
        }
    }
    Rendered {
        image: Some(renderer.finish()),
        diagnostics,
    }
}

/// The image size and the view of the map, or `None` with an error.
fn view(map: &Map, diagnostics: &mut Vec<Diagnostic>) -> Option<(u32, u32, View)> {
    let span = map.span.clone();
    let Some([width, height]) = map.size else {
        diagnostics.push(Diagnostic::error(
            "size-missing",
            "the map has no SIZE",
            span,
        ));
        return None;
    };
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        let message =
            format!("the image of {width} × {height} exceeds {MAX_IMAGE_SIDE} pixels a side");
        diagnostics.push(Diagnostic::error("image-too-large", message, span));
        return None;
    }
    let Some(extent) = map.extent else {
        diagnostics.push(Diagnostic::error(
            "extent-missing",
            "the map has no EXTENT",
            span,
        ));
        return None;
    };
    let Some(view) = View::fit(extent, width, height) else {
        let message = "the map's EXTENT and SIZE give no scale";
        diagnostics.push(Diagnostic::error("view-invalid", message, span));
        return None;
    };
    Some((width, height, view))
}

fn drawable_mode(layer: &Layer, diagnostics: &mut Vec<Diagnostic>) -> Option<RenderMode> {
    match layer.render_mode {
        Some(mode @ (RenderMode::Point | RenderMode::Line | RenderMode::Polygon)) => Some(mode),
        other => {
            let what = other.map_or("no TYPE".to_owned(), |m| format!("{m:?}").to_lowercase());
            let message = format!("layer {} is not drawn: {what}", name(layer));
            diagnostics.push(Diagnostic::warning(
                "layer-not-drawn",
                message,
                layer.span.clone(),
            ));
            None
        }
    }
}

async fn features<S: FeatureSource>(
    layer: &Layer,
    sources: &BTreeMap<String, S>,
    view: &View,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<Feature>> {
    let kind = layer.source.as_ref().map_or("none", |s| s.kind.as_str());
    let span = layer
        .source
        .as_ref()
        .and_then(|s| s.span.clone())
        .or_else(|| layer.span.clone());
    let skip = |reason: String| {
        let message = format!("layer {} (source {kind}) is skipped: {reason}", name(layer));
        Diagnostic::warning("layer-skipped", message, span.clone())
    };
    let Some(source) = layer.name.as_ref().and_then(|n| sources.get(n)) else {
        diagnostics.push(skip("no source is bound to it".to_owned()));
        return None;
    };
    let query = FeatureQuery {
        extent: view.extent,
    };
    match source.features(&query).await {
        Ok(features) => Some(features),
        Err(e) => {
            diagnostics.push(skip(e.message));
            None
        }
    }
}

fn name(layer: &Layer) -> String {
    layer
        .name
        .as_deref()
        .map_or("without NAME".to_owned(), |n| format!("`{n}`"))
}

/// The drawing operations of one feature in one style. The render mode decides how a geometry
/// is drawn: polygons in a line layer are drawn as their outlines; what a mode cannot draw is
/// left out.
fn draw_ops(mode: RenderMode, geometry: &Geometry, style: &Style, view: &View) -> Vec<DrawOp> {
    let width = style.width.unwrap_or(1.0);
    let pixels = |cs: &[Coord]| cs.iter().map(|&c| view.to_pixel(c)).collect::<Vec<_>>();
    let mut ops = Vec::new();
    match (mode, geometry) {
        (_, Geometry::Collection(geometries)) => {
            for g in geometries {
                ops.extend(draw_ops(mode, g, style, view));
            }
        }
        (RenderMode::Polygon, Geometry::Polygon(rings)) => {
            polygon_ops(
                &mut ops,
                rings.iter().map(|r| pixels(r)).collect(),
                style,
                width,
            );
        }
        (RenderMode::Polygon, Geometry::MultiPolygon(polygons)) => {
            for rings in polygons {
                polygon_ops(
                    &mut ops,
                    rings.iter().map(|r| pixels(r)).collect(),
                    style,
                    width,
                );
            }
        }
        (RenderMode::Line, Geometry::LineString(line)) => {
            stroke(&mut ops, vec![pixels(line)], false, style.color, width);
        }
        (RenderMode::Line, Geometry::MultiLineString(lines) | Geometry::Polygon(lines)) => {
            let closed = matches!(geometry, Geometry::Polygon(_));
            stroke(
                &mut ops,
                lines.iter().map(|l| pixels(l)).collect(),
                closed,
                style.color,
                width,
            );
        }
        (RenderMode::Line, Geometry::MultiPolygon(polygons)) => {
            let rings = polygons.iter().flatten().map(|r| pixels(r)).collect();
            stroke(&mut ops, rings, true, style.color, width);
        }
        (RenderMode::Point, Geometry::Point(c)) => point_ops(&mut ops, view.to_pixel(*c), style),
        (RenderMode::Point, Geometry::MultiPoint(cs)) => {
            for &c in cs {
                point_ops(&mut ops, view.to_pixel(c), style);
            }
        }
        _ => {}
    }
    ops
}

fn polygon_ops(ops: &mut Vec<DrawOp>, rings: Vec<Vec<Point>>, style: &Style, width: f64) {
    if let Some(color) = style.color {
        ops.push(DrawOp::Fill {
            rings: rings.clone(),
            color,
        });
    }
    stroke(ops, rings, true, style.outline_color, width);
}

fn stroke(
    ops: &mut Vec<DrawOp>,
    paths: Vec<Vec<Point>>,
    closed: bool,
    color: Option<Rgba>,
    width: f64,
) {
    if let Some(color) = color {
        ops.push(DrawOp::Stroke {
            paths,
            closed,
            color,
            width,
        });
    }
}

/// A point as a square of the style's size, centred on it.
fn point_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style) {
    let half = style.size.unwrap_or(1.0) / 2.0;
    let square = vec![
        Point {
            x: at.x - half,
            y: at.y - half,
        },
        Point {
            x: at.x + half,
            y: at.y - half,
        },
        Point {
            x: at.x + half,
            y: at.y + half,
        },
        Point {
            x: at.x - half,
            y: at.y + half,
        },
    ];
    polygon_ops(ops, vec![square], style, style.width.unwrap_or(1.0));
}
