//! The WebAssembly bindings: a host for the browser. A page hands in a mapfile, GeoJSON per layer
//! name, and a viewport, and gets back the RGBA image and the diagnostics (G-5).
//!
//! [`render_map`] holds the logic and is tested natively; [`render`] is its JavaScript face.

mod credentials;

use std::collections::BTreeMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};

use forager_adapter_geojson::GeoJsonSource;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_render_agg::AggRenderer;
use forager_style_ir::{Diagnostic, Extent};
use wasm_bindgen::prelude::wasm_bindgen;

/// The name the mapfile is known by in diagnostics.
const MAPFILE_NAME: &str = "map.map";

/// What a page may override of the mapfile's MAP block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    pub size: Option<[u32; 2]>,
    pub extent: Option<Extent>,
}

/// The image, unless the map could not be drawn at all, and every diagnostic.
#[derive(Debug)]
pub struct Output {
    pub image: Option<RgbaImage>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Renders `mapfile` with the GeoJSON in `sources`, a JSON object from layer name to a GeoJSON
/// object or to GeoJSON text. Source references that carry credentials are dropped (Q-7).
pub fn render_map(mapfile: &str, sources: &str, viewport: &Viewport) -> Output {
    let mut diagnostics = Vec::new();
    let Some(loaded) = now(forager_mapfile::load(
        &ResourcePath::new(MAPFILE_NAME).expect("a plain file name is a resource path"),
        &OneFile(mapfile),
    )) else {
        return waited();
    };
    diagnostics.extend(loaded.diagnostics);
    let Some(document) = loaded.document else {
        return Output {
            image: None,
            diagnostics,
        };
    };
    let mut map = document.map;
    let mut rejected = Vec::new();
    for layer in &mut map.layers {
        if layer
            .source
            .as_ref()
            .is_some_and(|source| !credentials::admissible(source))
        {
            let source = layer.source.take().expect("checked just above");
            rejected.extend(layer.name.clone());
            let message =
                "only DATA without credentials reaches the browser; the source is dropped";
            diagnostics.push(Diagnostic::error(
                "source-rejected",
                message,
                source.span.or_else(|| layer.span.clone()),
            ));
        }
    }
    if let Some(size) = viewport.size {
        map.size = Some(size);
    }
    if let Some(extent) = viewport.extent {
        map.extent = Some(extent);
    }
    let mut bound = bind(sources, &mut diagnostics);
    // A rejected layer gets no data either, so the engine skips it.
    for name in &rejected {
        bound.remove(name);
    }
    let Some(rendered) = now(forager_engine::render(&map, &bound, AggRenderer::new())) else {
        return waited();
    };
    diagnostics.extend(rendered.diagnostics);
    Output {
        image: rendered.image,
        diagnostics,
    }
}

/// Parses the sources; one that cannot be read is reported and left unbound, so the engine skips
/// its layer (G-8).
fn bind(sources: &str, diagnostics: &mut Vec<Diagnostic>) -> BTreeMap<String, GeoJsonSource> {
    let mut bound = BTreeMap::new();
    let entries = match serde_json::from_str::<serde_json::Value>(sources) {
        Ok(serde_json::Value::Object(entries)) => entries,
        _ => {
            let message = "sources are a JSON object from layer name to GeoJSON";
            diagnostics.push(Diagnostic::error("sources-invalid", message, None));
            return bound;
        }
    };
    for (layer, value) in entries {
        let text = match value {
            serde_json::Value::String(text) => text,
            other => other.to_string(),
        };
        match GeoJsonSource::parse(&text) {
            Ok(source) => {
                bound.insert(layer, source);
            }
            Err(e) => {
                let message = format!("source of layer `{layer}`: {e}");
                diagnostics.push(Diagnostic::warning("source-unreadable", message, None));
            }
        }
    }
    bound
}

/// The mapfile as the only file of the base context; an `INCLUDE` finds nothing.
struct OneFile<'a>(&'a str);

impl ResourceLoader for OneFile<'_> {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str() == MAPFILE_NAME {
            Ok(self.0.as_bytes().to_vec())
        } else {
            Err(SourceError::new(
                "the browser example has no files besides the mapfile",
            ))
        }
    }
}

/// Runs a future whose ports never wait — the mapfile and the GeoJSON are in memory — in one
/// poll. The browser is single-threaded, so nothing could wake a future that waits; `None` then,
/// rather than a spinning tab.
fn now<F: Future>(future: F) -> Option<F::Output> {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

fn waited() -> Output {
    let message = "a port waited, which the browser bindings cannot";
    Output {
        image: None,
        diagnostics: vec![Diagnostic::error("port-waited", message, None)],
    }
}

/// A rendered image and its diagnostics, for JavaScript.
#[wasm_bindgen]
pub struct RenderResult {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    diagnostics: String,
}

#[wasm_bindgen]
impl RenderResult {
    /// The image width in pixels, 0 when the map could not be drawn.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The straight-alpha RGBA bytes, row by row, ready for an `ImageData`.
    #[wasm_bindgen(getter)]
    pub fn pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }

    /// The diagnostics as a JSON array of `{severity, code, message, span?}`.
    #[wasm_bindgen(getter)]
    pub fn diagnostics(&self) -> String {
        self.diagnostics.clone()
    }
}

/// Renders a mapfile in the browser. `sources` is a JSON object from layer name to GeoJSON (an
/// object or its text). `width`, `height`, and `extent` (`[minx, miny, maxx, maxy]`) override
/// the mapfile's SIZE and EXTENT when given.
#[wasm_bindgen]
pub fn render(
    mapfile: &str,
    sources: &str,
    width: Option<u32>,
    height: Option<u32>,
    extent: Option<Vec<f64>>,
) -> RenderResult {
    let mut diagnostics = Vec::new();
    let extent = match extent.as_deref() {
        None => None,
        Some(&[min_x, min_y, max_x, max_y]) => Some(Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        }),
        Some(_) => {
            let message = "an extent is [minx, miny, maxx, maxy]";
            diagnostics.push(Diagnostic::error("viewport-invalid", message, None));
            None
        }
    };
    let viewport = Viewport {
        size: match (width, height) {
            (Some(w), Some(h)) => Some([w, h]),
            (None, None) => None,
            _ => {
                let message = "width and height are given together";
                diagnostics.push(Diagnostic::error("viewport-invalid", message, None));
                None
            }
        },
        extent,
    };
    let output = render_map(mapfile, sources, &viewport);
    diagnostics.extend(output.diagnostics);
    let image = output.image.unwrap_or(RgbaImage {
        width: 0,
        height: 0,
        pixels: Vec::new(),
    });
    RenderResult {
        width: image.width,
        height: image.height,
        pixels: image.pixels,
        diagnostics: serde_json::to_string(&diagnostics).unwrap_or_else(|_| "[]".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE_MAP: &str = include_str!("../../cli/tests/fixtures/square.map");
    const AREAS: &str = include_str!("../../cli/tests/fixtures/areas.geojson");

    fn codes(output: &Output) -> Vec<&str> {
        output.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn the_cli_fixture_renders_to_the_engines_reference_image() {
        // Verifies: G-5
        let sources = serde_json::json!({ "areas": AREAS }).to_string();
        let output = render_map(SQUARE_MAP, &sources, &Viewport::default());
        assert_eq!(
            codes(&output),
            ["layer-skipped"],
            "the fixture's `unbound` layer"
        );
        let reference =
            forager_encode::decode_png(include_bytes!("../../engine/tests/fixtures/polygon.png"))
                .unwrap();
        assert!(output.image.unwrap() == reference);
    }

    #[test]
    fn a_source_with_credentials_is_rejected() {
        // Verifies: Q-7
        let mapfile = SQUARE_MAP.replace(
            "DATA \"areas.geojson\"",
            "CONNECTIONTYPE POSTGIS CONNECTION \"host=db user=u password=secret\" DATA \"geom from t\"",
        );
        let sources = serde_json::json!({ "areas": AREAS }).to_string();
        let output = render_map(&mapfile, &sources, &Viewport::default());
        let rejected = &output.diagnostics[0];
        assert_eq!(rejected.code, "source-rejected");
        assert!(!rejected.message.contains("secret"));
        let image = output.image.unwrap();
        assert_eq!(
            image.pixel(25, 75),
            Some(forager_style_ir::Rgba::opaque(255, 255, 255))
        );
    }

    #[test]
    fn the_viewport_overrides_size_and_extent() {
        let sources = serde_json::json!({ "areas": serde_json::from_str::<serde_json::Value>(AREAS).unwrap() }).to_string();
        let viewport = Viewport {
            size: Some([11, 11]),
            extent: Some(Extent {
                min_x: 40.0,
                min_y: 40.0,
                max_x: 60.0,
                max_y: 60.0,
            }),
        };
        let image = render_map(SQUARE_MAP, &sources, &viewport).image.unwrap();
        assert_eq!((image.width, image.height), (11, 11));
        assert_eq!(
            image.pixel(5, 5),
            Some(forager_style_ir::Rgba::opaque(255, 255, 255)),
            "the hole"
        );
    }

    #[test]
    fn broken_inputs_are_diagnostics() {
        // Verifies: Q-3
        assert_eq!(
            codes(&render_map("MAP", "{}", &Viewport::default())),
            ["unterminated-block"]
        );
        let output = render_map(SQUARE_MAP, "[1]", &Viewport::default());
        assert_eq!(output.diagnostics[0].code, "sources-invalid");
        let output = render_map(SQUARE_MAP, r#"{"areas": "{"}"#, &Viewport::default());
        assert_eq!(output.diagnostics[0].code, "source-unreadable");
        let result = render("MAP END", "{}", None, None, Some(vec![1.0]));
        assert!(result.diagnostics().contains("viewport-invalid"));
        assert_eq!(result.width(), 0);
        let result = render(SQUARE_MAP, "{}", Some(10), None, None);
        assert!(result.diagnostics().contains("viewport-invalid"));
    }
}

#[cfg(test)]
mod example {
    use super::*;

    /// The page under `examples/web/` passes these files to `render`; they render clean.
    #[test]
    fn the_browser_example_renders_without_diagnostics() {
        let sources = serde_json::json!({
            "areas": include_str!("../../../examples/web/areas.geojson"),
            "roads": include_str!("../../../examples/web/roads.geojson"),
            "places": include_str!("../../../examples/web/places.geojson"),
        })
        .to_string();
        let output = render_map(
            include_str!("../../../examples/web/sample.map"),
            &sources,
            &Viewport::default(),
        );
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let image = output.image.unwrap();
        assert_eq!((image.width, image.height), (640, 400));
    }
}
