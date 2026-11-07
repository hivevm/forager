//! The WebAssembly bindings: a host for the browser. A page hands in a mapfile, GeoJSON per layer
//! name, a viewport, and the files the mapfile names, such as its font set, its fonts, and the
//! images of its pixmap symbols, and gets back the RGBA image and the diagnostics (G-5).
//!
//! [`render_map`] holds the logic and is tested natively; [`render`] is its JavaScript face.

mod credentials;

use std::collections::BTreeMap;

use forager_adapter_epsg::EpsgTable;
use forager_adapter_geojson::GeoJsonSource;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_render_agg::AggRenderer;
use forager_style_ir::{Diagnostic, Extent, Map};
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
/// `init=` projections resolve with the built-in EPSG codes and those of `epsg`, a table in the
/// format of PROJ's former `epsg` init file (ADR-0023). The mapfile is the only file of its base
/// context.
pub fn render_map(mapfile: &str, sources: &str, epsg: Option<&str>, viewport: &Viewport) -> Output {
    render_map_with_files(mapfile, sources, epsg, viewport, &BTreeMap::new())
}

/// Renders as [`render_map`] does, with `files` beside the mapfile in its base context, by the
/// path the mapfile names them: a font set and its fonts, an `INCLUDE`, a symbol set (ADR-0028),
/// the image of a pixmap symbol (ADR-0040). Nothing else is read; a font or an image past
/// [`forager_engine::MAX_FONT_BYTES`] is refused by the engine.
pub fn render_map_with_files(
    mapfile: &str,
    sources: &str,
    epsg: Option<&str>,
    viewport: &Viewport,
    files: &BTreeMap<String, Vec<u8>>,
) -> Output {
    let base = PageFiles { mapfile, files };
    let mut diagnostics = Vec::new();
    let resolver = match epsg.map(EpsgTable::with_table) {
        None => EpsgTable::new(),
        Some(Ok(table)) => table,
        Some(Err(errors)) => {
            for e in errors {
                let message = format!("EPSG table, {e}");
                diagnostics.push(Diagnostic::error("epsg-table-invalid", message, None));
            }
            return Output {
                image: None,
                diagnostics,
            };
        }
    };
    let Some(loaded) = forager_ports::poll_once(forager_mapfile::load(
        &ResourcePath::new(MAPFILE_NAME).expect("a plain file name is a resource path"),
        &base,
        Some(&resolver),
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
    let mut bound = bind(&map, sources, &mut diagnostics);
    // A rejected layer gets no data either, so the engine skips it.
    for name in &rejected {
        bound.remove(name);
    }
    let Some(rendered) = forager_ports::poll_once(forager_engine::render_with_files(
        &map,
        &bound,
        &base,
        AggRenderer::new(),
    )) else {
        return waited();
    };
    diagnostics.extend(rendered.diagnostics);
    Output {
        image: rendered.image,
        diagnostics,
    }
}

/// Binds the sources to the layers of `map` by name, as every host does
/// ([`forager_engine::bind_sources`]).
fn bind(
    map: &Map,
    sources: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, GeoJsonSource> {
    let entries = match serde_json::from_str::<serde_json::Value>(sources) {
        Ok(serde_json::Value::Object(entries)) => entries,
        _ => {
            let message = "sources are a JSON object from layer name to GeoJSON";
            diagnostics.push(Diagnostic::error("sources-invalid", message, None));
            return BTreeMap::new();
        }
    };
    forager_engine::bind_sources(
        map,
        entries,
        |value| {
            let text = match value {
                serde_json::Value::String(text) => text,
                other => other.to_string(),
            };
            GeoJsonSource::parse(&text).map_err(|e| e.to_string())
        },
        diagnostics,
    )
}

/// The base context of a render in the browser: the mapfile, and the files the page hands in by
/// their paths; nothing else exists in it.
struct PageFiles<'a> {
    mapfile: &'a str,
    files: &'a BTreeMap<String, Vec<u8>>,
}

impl ResourceLoader for PageFiles<'_> {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str() == MAPFILE_NAME {
            return Ok(self.mapfile.as_bytes().to_vec());
        }
        self.files
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| SourceError::new("the page handed in no file of this path"))
    }
}

/// The files a page hands in beside the mapfile, each by the path the mapfile names it.
#[wasm_bindgen]
#[derive(Default)]
pub struct Files {
    files: BTreeMap<String, Vec<u8>>,
}

#[wasm_bindgen]
impl Files {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Files {
        Files::default()
    }

    /// Adds the file at `path`, a path relative to the base context as the mapfile names it, with
    /// its bytes; a later one of the same path replaces it. A path that is no resource path, such
    /// as one that climbs out of the base context, is not added, and the call returns `false`.
    pub fn add(&mut self, path: &str, bytes: Vec<u8>) -> bool {
        match ResourcePath::new(path) {
            Ok(path) => {
                self.files.insert(path.as_str().to_owned(), bytes);
                true
            }
            Err(_) => false,
        }
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
/// the mapfile's SIZE and EXTENT when given. `epsg` is a table of further EPSG definitions
/// (ADR-0023). The mapfile is the only file of its base context; [`render_with`] adds others.
#[wasm_bindgen]
pub fn render(
    mapfile: &str,
    sources: &str,
    width: Option<u32>,
    height: Option<u32>,
    extent: Option<Vec<f64>>,
    epsg: Option<String>,
) -> RenderResult {
    render_with(
        mapfile,
        sources,
        width,
        height,
        extent,
        epsg,
        &Files::default(),
    )
}

/// Renders as [`render`] does, with `files` beside the mapfile, such as its font set and fonts.
/// The page keeps `files` and may hand it to every render, of every tile.
#[wasm_bindgen(js_name = renderWith)]
pub fn render_with(
    mapfile: &str,
    sources: &str,
    width: Option<u32>,
    height: Option<u32>,
    extent: Option<Vec<f64>>,
    epsg: Option<String>,
    files: &Files,
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
    let output = render_map_with_files(mapfile, sources, epsg.as_deref(), &viewport, &files.files);
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

    /// A page hands in the font set and the fonts the mapfile names; the engine reads them from
    /// there, and from nowhere else.
    #[test]
    fn the_fonts_a_page_hands_in_reach_the_engine() {
        // Verifies: ADR-0028
        let mapfile = |font: &str| {
            format!(
                "MAP SIZE 10 10 EXTENT 0 0 10 10 FONTSET 'fonts.lst'\n\
                 LAYER NAME 'p' TYPE POINT STATUS ON\n\
                 CLASS LABEL FONT '{font}' TEXT 'x' END END END END\n"
            )
        };
        let sources = serde_json::json!({ "p": {"type": "Point", "coordinates": [5, 5]} });
        let sources = sources.to_string();
        let mut page = Files::new();
        assert!(page.add(
            "fonts.lst",
            b"sans fonts/sans.ttf\ngone fonts/gone.ttf\n".to_vec()
        ));
        // A path is normalized as the mapfile's paths are; one out of the base context is refused.
        // A test font of the label crate, a subset of DejaVu Sans whose licence lies beside it.
        let sans = include_bytes!("../../label/tests/fixtures/dejavu-sans-subset.ttf");
        assert!(page.add("./fonts/sans.ttf", sans.to_vec()));
        assert!(!page.add("../escape.ttf", b"font".to_vec()));
        let render = |font: &str| {
            render_map_with_files(
                &mapfile(font),
                &sources,
                None,
                &Viewport::default(),
                &page.files,
            )
        };
        assert_eq!(codes(&render("sans")), Vec::<&str>::new());
        let output = render("gone");
        assert_eq!(codes(&output), ["label-font-unusable"]);
        assert!(output.diagnostics[0].message.contains("fonts/gone.ttf"));
        // Without the files, the font set itself is not found.
        let without = render_map(&mapfile("sans"), &sources, None, &Viewport::default());
        assert_eq!(codes(&without), ["resource-unreadable"]);
    }

    /// A `DATA` with a character of more than one byte reaches the credential filter, and the
    /// render returns its diagnostics instead of aborting the page.
    #[test]
    fn a_data_with_a_multi_byte_character_renders_without_a_panic() {
        // Verifies: Q-3
        let mapfile = "MAP SIZE 10 10 EXTENT 0 0 10 10\n\
                       LAYER NAME 'p' TYPE POINT STATUS ON DATA 'ü=1.shp'\n\
                       CLASS STYLE COLOR 0 0 0 SIZE 3 END END END END\n";
        let sources = serde_json::json!({ "p": {"type": "Point", "coordinates": [5, 5]} });
        let output = render_map(mapfile, &sources.to_string(), None, &Viewport::default());
        assert!(output.image.is_some());
        assert!(
            !codes(&output).contains(&"source-rejected"),
            "{:?}",
            codes(&output)
        );
    }

    /// A shapefile among the page's files binds no layer: the `DATA` layer that names it is
    /// skipped as unbound, so the bytes of its files need not be a shapefile.
    #[test]
    fn a_shapefile_among_the_pages_files_leaves_its_layer_skipped() {
        // Verifies: ADR-0038
        let mapfile = "MAP SIZE 10 10 EXTENT 0 0 10 10\n\
                       LAYER NAME 'roads' TYPE POINT STATUS ON DATA 'data/roads.shp'\n\
                       CLASS STYLE COLOR 0 0 0 SIZE 3 END END END END\n";
        let mut page = Files::new();
        for extension in ["shp", "shx", "dbf"] {
            assert!(page.add(&format!("data/roads.{extension}"), b"never read".to_vec()));
        }
        let output = render_map_with_files(mapfile, "{}", None, &Viewport::default(), &page.files);
        assert_eq!(codes(&output), ["layer-skipped"]);
        let message = &output.diagnostics[0].message;
        assert!(message.contains("no source is bound to it"), "{message}");
        assert!(output.image.is_some());
    }

    #[test]
    fn the_cli_fixture_renders_to_the_engines_reference_image() {
        // Verifies: G-5
        let sources = serde_json::json!({ "areas": AREAS }).to_string();
        let output = render_map(SQUARE_MAP, &sources, None, &Viewport::default());
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

    /// A page hands in the image of a pixmap symbol beside the mapfile, by the path the mapfile
    /// names; the engine draws it as the CLI draws pixmap.map, whose test reads the same
    /// reference image of the engine.
    #[test]
    fn a_pixmap_a_page_hands_in_is_drawn_as_the_cli_draws_it() {
        // Verifies: G-5
        let mapfile = include_str!("../../cli/tests/fixtures/pixmap.map");
        let mut page = Files::new();
        assert!(page.add(
            "pin.png",
            include_bytes!("../../cli/tests/fixtures/pin.png").to_vec()
        ));
        let output = render_map_with_files(mapfile, "{}", None, &Viewport::default(), &page.files);
        assert_eq!(codes(&output), Vec::<&str>::new());
        let reference = forager_encode::decode_png(include_bytes!(
            "../../engine/tests/fixtures/pixmap_point.png"
        ))
        .unwrap();
        assert!(output.image.unwrap() == reference);
        // Without the image, the style is not drawn, and the map is.
        let output = render_map(mapfile, "{}", None, &Viewport::default());
        assert_eq!(codes(&output), ["pixmap-not-drawn"]);
        assert!(output.diagnostics[0].message.contains("pin.png"));
        assert!(output.image.is_some());
    }

    /// The bindings' loader holds every file a page hands in; the engine refuses an image past
    /// its bound on a file itself.
    #[test]
    fn a_pixmap_file_past_32_mib_is_not_drawn_and_the_map_is() {
        // Verifies: Q-9
        let mapfile = "MAP SIZE 21 21 EXTENT 0 0 20 20 IMAGECOLOR 255 255 255\n\
                       SYMBOL NAME 'big' TYPE PIXMAP IMAGE 'big.png' END\n\
                       LAYER NAME 'p' TYPE POINT STATUS ON FEATURE POINTS 10 10 END END\n\
                       CLASS STYLE SYMBOL 'big' END STYLE COLOR 0 0 255 SIZE 5 END END END END\n";
        let mut page = Files::new();
        assert!(page.add("big.png", vec![0; forager_engine::MAX_FONT_BYTES + 1]));
        let output = render_map_with_files(mapfile, "{}", None, &Viewport::default(), &page.files);
        assert_eq!(codes(&output), ["pixmap-not-drawn"]);
        let message = &output.diagnostics[0].message;
        assert!(
            message.contains("`big`") && message.contains("big.png"),
            "{message}"
        );
        assert!(
            message.contains("exceeds the limit of 33554432 bytes"),
            "{message}"
        );
        let image = output.image.unwrap();
        assert_eq!(
            image.pixel(10, 10),
            Some(forager_style_ir::Rgba::opaque(0, 0, 255))
        );
        assert_eq!(
            image.pixel(1, 1),
            Some(forager_style_ir::Rgba::opaque(255, 255, 255))
        );
    }

    #[test]
    fn a_source_for_no_layer_is_reported() {
        // Verifies: G-8
        let sources = serde_json::json!({ "areas": AREAS, "area": AREAS }).to_string();
        let output = render_map(SQUARE_MAP, &sources, None, &Viewport::default());
        let unbound: Vec<_> = output
            .diagnostics
            .iter()
            .filter(|d| d.code == "source-unbound")
            .collect();
        assert_eq!(unbound.len(), 1, "{:?}", output.diagnostics);
        assert!(
            unbound[0].message.contains("`area`"),
            "{}",
            unbound[0].message
        );
    }

    #[test]
    fn the_cli_and_the_bindings_report_the_same_codes() {
        // The CLI's test of the same inputs reads the same expectation, in
        // `crates/cli/tests/render.rs`.
        let expected: Vec<_> = include_str!("../../cli/tests/fixtures/misbound.codes")
            .lines()
            .collect();
        let sources =
            serde_json::json!({ "unbound": "{", "areas": AREAS, "area": AREAS }).to_string();
        let output = render_map(SQUARE_MAP, &sources, None, &Viewport::default());
        assert_eq!(codes(&output), expected);
    }

    #[test]
    fn a_source_with_credentials_is_rejected() {
        // Verifies: Q-7
        let mapfile = SQUARE_MAP.replace(
            "DATA \"areas.geojson\"",
            "CONNECTIONTYPE POSTGIS CONNECTION \"host=db user=u password=secret\" DATA \"geom from t\"",
        );
        let sources = serde_json::json!({ "areas": AREAS }).to_string();
        let output = render_map(&mapfile, &sources, None, &Viewport::default());
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
        let image = render_map(SQUARE_MAP, &sources, None, &viewport)
            .image
            .unwrap();
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
            codes(&render_map("MAP", "{}", None, &Viewport::default())),
            ["unterminated-block"]
        );
        let output = render_map(SQUARE_MAP, "[1]", None, &Viewport::default());
        assert_eq!(output.diagnostics[0].code, "sources-invalid");
        let output = render_map(SQUARE_MAP, r#"{"areas": "{"}"#, None, &Viewport::default());
        assert_eq!(output.diagnostics[0].code, "source-unreadable");
        let result = render("MAP END", "{}", None, None, Some(vec![1.0]), None);
        assert!(result.diagnostics().contains("viewport-invalid"));
        assert_eq!(result.width(), 0);
        let result = render(SQUARE_MAP, "{}", Some(10), None, None, None);
        assert!(result.diagnostics().contains("viewport-invalid"));
    }
}

#[cfg(test)]
mod example {
    use super::*;

    const MAPFILE: &str = include_str!("../../../examples/web/sample.map");

    /// The layers' GeoJSON the page under `examples/web/` binds by name, which
    /// `scripts/generate-example.sh` writes.
    fn sources() -> String {
        serde_json::json!({
            "landuse": include_str!("../../../examples/web/landuse.geojson"),
            "rivers": include_str!("../../../examples/web/rivers.geojson"),
            "buildings": include_str!("../../../examples/web/buildings.geojson"),
            "roads": include_str!("../../../examples/web/roads.geojson"),
            "places": include_str!("../../../examples/web/places.geojson"),
        })
        .to_string()
    }

    /// The files the page hands in beside the mapfile, its `FILES`: the font set and its font.
    fn files() -> BTreeMap<String, Vec<u8>> {
        let mut page = Files::new();
        assert!(page.add(
            "fonts.lst",
            include_bytes!("../../../examples/web/fonts.lst").to_vec()
        ));
        assert!(page.add(
            "vendor/dejavu/DejaVuSans.ttf",
            include_bytes!("../../../examples/web/vendor/dejavu/DejaVuSans.ttf").to_vec()
        ));
        page.files
    }

    /// The page passes these files to `renderWith`; they render clean.
    #[test]
    fn the_browser_example_renders_without_diagnostics() {
        let output =
            render_map_with_files(MAPFILE, &sources(), None, &Viewport::default(), &files());
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let image = output.image.unwrap();
        assert_eq!((image.width, image.height), (640, 400));
    }

    /// The page's tile view renders each tile with its edges moved in by half a pixel, and with
    /// a buffer of `TILE_BUFFER` pixels around it that it cuts away, so that a label beside a
    /// tile's edge is placed as its neighbour places it, as long as a label and the labels it
    /// gives way to reach less far than the buffer; two neighbouring tiles then match one image
    /// of both, drawn the same way, within rounding. A tile that cuts a dashed line starts its
    /// dashes again at the cut, as MapServer clips a line before it dashes it, and a label the
    /// buffer does not hold may stand otherwise; the edges below are chosen clear of both, which
    /// the test cannot hold for every edge.
    #[test]
    fn neighbouring_tiles_meet_without_a_seam() {
        // The page's `TILE_BUFFER`, and Leaflet's tile of 256 pixels.
        const BUFFER: u32 = 256;
        const TILE: u32 = 256;
        let (sources, files) = (sources(), files());
        // The image of `tiles` tiles across from (min_x, min_y), each `unit` map units wide.
        let draw = |min_x: f64, min_y: f64, unit: f64, tiles: u32| {
            let pixel = unit / f64::from(TILE);
            let grown = pixel * f64::from(BUFFER);
            let viewport = Viewport {
                size: Some([tiles * TILE + 2 * BUFFER, TILE + 2 * BUFFER]),
                extent: Some(Extent {
                    min_x: min_x - grown + pixel / 2.0,
                    min_y: min_y - grown + pixel / 2.0,
                    max_x: min_x + f64::from(tiles) * unit + grown - pixel / 2.0,
                    max_y: min_y + unit + grown - pixel / 2.0,
                }),
            };
            let output = render_map_with_files(MAPFILE, &sources, None, &viewport, &files);
            assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
            (output.image.unwrap(), (tiles * TILE + 2 * BUFFER) as usize)
        };
        let at = |(image, stride): &(RgbaImage, usize), x: usize, y: usize| {
            let i = ((y + BUFFER as usize) * stride + x + BUFFER as usize) * 4;
            image.pixels[i..i + 4].to_vec()
        };
        // The vertical edges between two tiles of Leaflet's grid, as (zoom, x, y) of the edge's
        // lower end in map units, a tile 2^-zoom × 256 units wide. At zoom 0, where the page
        // starts, the edge at x 2048 runs through the middle of the map; at zoom 2, two levels
        // deeper, the edge at x 1344 runs 8 units, 32 pixels, left of the point of Fernmere, the
        // largest place. Without the buffer, both edges show a seam: the labels the cache places
        // differ.
        for (zoom, edge, min_y) in [(0, 2048.0, 1024.0), (2, 1344.0, 768.0)] {
            let unit = f64::from(TILE) / 2f64.powi(zoom);
            let whole = draw(edge - unit, min_y, unit, 2);
            let tiles = [
                draw(edge - unit, min_y, unit, 1),
                draw(edge, min_y, unit, 1),
            ];
            for y in 0..TILE as usize {
                for x in 0..2 * TILE as usize {
                    let tile = &tiles[x / TILE as usize];
                    let (p, q) = (at(&whole, x, y), at(tile, x % TILE as usize, y));
                    for (p, q) in p.iter().zip(&q) {
                        assert!(
                            p.abs_diff(*q) <= 1,
                            "zoom {zoom}, edge {edge}, pixel ({x}, {y}): {p} against {q}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod projections {
    use super::*;

    const GRID_MAP: &str = "MAP PROJECTION 'init=epsg:27700' END END";

    #[test]
    fn the_browser_resolves_built_in_codes_and_takes_a_table_for_others() {
        // Verifies: ADR-0023
        let wgs84 = render_map(
            "MAP SIZE 10 10 EXTENT 0 0 1 1 PROJECTION 'init=epsg:4326' END END",
            "{}",
            None,
            &Viewport::default(),
        );
        assert!(wgs84.diagnostics.is_empty(), "{:?}", wgs84.diagnostics);

        let without = render_map(GRID_MAP, "{}", None, &Viewport::default());
        assert_eq!(without.diagnostics[0].code, "projection-unresolved");

        let table = "<27700> +proj=tmerc +lat_0=49 +lon_0=-2 +ellps=airy +units=m +no_defs <>";
        let with = render_map(GRID_MAP, "{}", Some(table), &Viewport::default());
        assert!(
            with.diagnostics
                .iter()
                .all(|d| d.code != "projection-unresolved"),
            "{:?}",
            with.diagnostics
        );

        let broken = render_map(
            GRID_MAP,
            "{}",
            Some("27700 +proj=tmerc"),
            &Viewport::default(),
        );
        assert_eq!(broken.diagnostics[0].code, "epsg-table-invalid");
    }
}
