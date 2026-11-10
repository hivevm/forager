//! The comparison of G-9 (ADR-0034): a case's map, its shapefiles' features written into its
//! layers (ADR-0018), rendered natively and by the browser bindings' wasm32 build, which
//! `crates/bindings-wasm` compiles with its feature `conformance` and wasmi runs here. The module's
//! exports and the format of its input and output are those `crates/bindings-wasm/src/
//! conformance.rs` states; it renders the document with no source bound and reads the files of
//! the native render, each as the native render's loader gave it.

use std::cell::RefCell;
use std::collections::BTreeMap;

use forager_adapter_geojson::GeoJsonSource;
use forager_adapter_shapefile::ShapefileSource;
use forager_ports::{
    FeatureQuery, FeatureSource, ResourceLoader, ResourcePath, SourceError, Value,
};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Extent, FeatureProperty, Geometry, InlineFeature, Map, ShapeType, StyleDocument,
};
use wasmi::{Engine, Linker, Module, Store, TypedFunc, Val};

/// The files a render read, by their paths: the bytes the loader gave, or its error.
pub type Files = BTreeMap<String, Result<Vec<u8>, String>>;

/// An image as the module gives it, or why there is none.
pub type Image = Result<(u32, u32, Vec<u8>), String>;

/// `map` with the features of each layer whose shapefile it reads written into the layer, all of
/// them, in the shapefile's order. The error names each shapefile that does not open or read,
/// which would leave its layer empty on both sides and the comparison empty with it.
pub fn inline(map: &mut Map, data: &impl ResourceLoader) -> Result<(), String> {
    let mut failed = Vec::new();
    let everywhere = FeatureQuery {
        extent: Extent {
            min_x: f64::MIN,
            min_y: f64::MIN,
            max_x: f64::MAX,
            max_y: f64::MAX,
        },
    };
    for layer in &mut map.layers {
        let Some(source) = &layer.source else {
            continue;
        };
        if !forager_engine::reads_bound_source(layer) || source.kind != "local" {
            continue;
        }
        let Some(path) = source.props.get("data") else {
            continue;
        };
        let read = ResourcePath::new(path)
            .map_err(|e| e.to_string())
            .and_then(|path| {
                let shapefile = forager_ports::poll_once(ShapefileSource::open(data, &path))
                    .ok_or("its loader waited")?
                    .map_err(|e| e.message)?;
                forager_ports::poll_once(shapefile.features(&everywhere))
                    .ok_or("its loader waited")?
                    .map_err(|e| e.message)
            });
        let features = match read {
            Ok(features) => features,
            Err(why) => {
                failed.push(format!("{path}: {why}"));
                continue;
            }
        };
        layer.features = features
            .features
            .iter()
            .filter_map(|feature| {
                let (shape_type, parts) = parts(&feature.geometry)?;
                let mut properties = Vec::new();
                feature.properties.each(|name, value| {
                    properties.push(FeatureProperty {
                        name: name.to_owned(),
                        value: match value {
                            Value::Null => String::new(),
                            Value::Bool(b) => u8::from(b).to_string(),
                            Value::Number(text) => text.to_owned(),
                            Value::String(text) => text.into_owned(),
                        },
                    });
                });
                Some(InlineFeature {
                    shape_type,
                    parts,
                    properties,
                    text: None,
                    span: None,
                })
            })
            .collect();
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("its shapefiles do not read: {}", failed.join("; ")))
    }
}

/// The shape of `geometry` as an inline feature holds it; a collection, which no shapefile gives,
/// has none.
fn parts(geometry: &Geometry) -> Option<(ShapeType, Vec<Vec<[f64; 2]>>)> {
    let line = |coords: &Vec<forager_style_ir::Coord>| coords.iter().map(|c| [c.x, c.y]).collect();
    Some(match geometry {
        Geometry::Point(c) => (ShapeType::Point, vec![vec![[c.x, c.y]]]),
        Geometry::MultiPoint(cs) => (ShapeType::Point, vec![line(cs)]),
        Geometry::LineString(cs) => (ShapeType::Line, vec![line(cs)]),
        Geometry::MultiLineString(ls) => (ShapeType::Line, ls.iter().map(line).collect()),
        Geometry::Polygon(rings) => (ShapeType::Polygon, rings.iter().map(line).collect()),
        Geometry::MultiPolygon(polygons) => (
            ShapeType::Polygon,
            polygons.iter().flatten().map(line).collect(),
        ),
        Geometry::Collection(_) => return None,
    })
}

/// A loader that keeps what it gave of each path.
struct Recording<'a, L> {
    loader: &'a L,
    files: RefCell<Files>,
}

impl<L: ResourceLoader> ResourceLoader for Recording<'_, L> {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        let loaded = self.loader.load(path).await;
        self.files.borrow_mut().insert(
            path.as_str().to_owned(),
            loaded.clone().map_err(|e| e.message.clone()),
        );
        loaded
    }
}

/// The native render of `document`, read from its JSON as the module reads it, with no source
/// bound and its files read through `files`, and the files it read.
pub fn native(document: &str, files: &impl ResourceLoader) -> (Image, Files) {
    let recording = Recording {
        loader: files,
        files: RefCell::new(Files::new()),
    };
    let image = (|| {
        let read = StyleDocument::from_json(document).map_err(|e| format!("the document: {e}"))?;
        let sources: BTreeMap<String, GeoJsonSource> = BTreeMap::new();
        let rendered = forager_ports::poll_once(forager_engine::render_with_files(
            &read.document.map,
            &sources,
            &recording,
            AggRenderer::new(),
        ))
        .ok_or("a port waited")?;
        let errors: Vec<_> = rendered
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect();
        let image = rendered.image.ok_or_else(|| errors.join("; "))?;
        Ok((image.width, image.height, image.pixels))
    })();
    (image, recording.files.into_inner())
}

/// The input of the module: the document's JSON and `files`, in the module's format.
fn input(document: &str, files: &Files) -> Vec<u8> {
    fn bytes(into: &mut Vec<u8>, b: &[u8]) {
        let len = u32::try_from(b.len()).expect("an input of less than 4 GiB");
        into.extend(len.to_le_bytes());
        into.extend(b);
    }
    let mut input = Vec::new();
    bytes(&mut input, document.as_bytes());
    let count = u32::try_from(files.len()).expect("fewer than 2^32 files");
    input.extend(count.to_le_bytes());
    for (path, file) in files {
        bytes(&mut input, path.as_bytes());
        match file {
            Ok(b) => {
                input.push(1);
                bytes(&mut input, b);
            }
            Err(e) => {
                input.push(0);
                bytes(&mut input, e.as_bytes());
            }
        }
    }
    input
}

/// The browser bindings' module built with the feature `conformance`, compiled once for wasmi.
pub struct Wasm {
    engine: Engine,
    module: Module,
}

impl Wasm {
    /// The module of `bytes`.
    pub fn new(bytes: &[u8]) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).map_err(|e| e.to_string())?;
        Ok(Self { engine, module })
    }

    /// The module's render of `document` with `files`, in a fresh instance. The module imports
    /// what wasm-bindgen's exports of the browser API import; the conformance exports call none of
    /// them, so each import is a function that traps.
    pub fn render(&self, document: &str, files: &Files) -> Result<Image, String> {
        let mut store = Store::new(&self.engine, ());
        let mut linker = Linker::<()>::new(&self.engine);
        for import in self.module.imports() {
            let Some(ty) = import.ty().func() else {
                return Err(format!(
                    "the module imports {}::{}, which is no function",
                    import.module(),
                    import.name()
                ));
            };
            let name = format!("{}::{}", import.module(), import.name());
            linker
                .func_new(
                    import.module(),
                    import.name(),
                    ty.clone(),
                    move |_, _: &[Val], _: &mut [Val]| {
                        Err(wasmi::Error::new(format!("the module called {name}")))
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        let instance = linker
            .instantiate_and_start(&mut store, &self.module)
            .map_err(|e| e.to_string())?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or("the module exports no memory")?;
        let func = |name: &str| -> Result<TypedFunc<(), u32>, String> {
            instance
                .get_typed_func(&store, name)
                .map_err(|e| format!("{name}: {e}"))
        };
        let (render, output) = (func("conformance_render")?, func("conformance_output")?);
        let reserve: TypedFunc<u32, u32> = instance
            .get_typed_func(&store, "conformance_input")
            .map_err(|e| format!("conformance_input: {e}"))?;

        let input = input(document, files);
        let len = u32::try_from(input.len()).map_err(|e| e.to_string())?;
        let at = reserve.call(&mut store, len).map_err(|e| e.to_string())?;
        memory
            .write(&mut store, at as usize, &input)
            .map_err(|e| e.to_string())?;
        let len = render.call(&mut store, ()).map_err(|e| e.to_string())?;
        let at = output.call(&mut store, ()).map_err(|e| e.to_string())?;
        let mut out = vec![0; len as usize];
        memory
            .read(&store, at as usize, &mut out)
            .map_err(|e| e.to_string())?;
        let number = |i: usize| u32::from_le_bytes([out[i], out[i + 1], out[i + 2], out[i + 3]]);
        if out.len() < 8 {
            return Err("the module's output is shorter than its header".to_owned());
        }
        let (width, height) = (number(0), number(4));
        Ok(if width == 0 {
            Err(String::from_utf8_lossy(&out[8..]).into_owned())
        } else {
            Ok((width, height, out[8..].to_vec()))
        })
    }
}

/// What differs between the native image and the module's: their sizes, the first pixel whose
/// bytes differ, by its column and row, or why either has no image; `None` when they are the
/// same, byte for byte. A case in scope has a reference image, so a native render that gives none
/// is a difference whatever the module gives, as two renders without an image compare nothing.
pub fn difference(native: &Image, wasm: &Image) -> Option<String> {
    match (native, wasm) {
        (Err(why), _) => Some(format!("the native render gives no image: {why}")),
        (Ok(_), Err(why)) => Some(format!("the module renders no image: {why}")),
        (Ok((w, h, a)), Ok((ww, wh, b))) => {
            if (w, h) != (ww, wh) || a.len() != b.len() {
                return Some(format!(
                    "the native image is {w}x{h}, the module's {ww}x{wh}"
                ));
            }
            let i = a.chunks(4).zip(b.chunks(4)).position(|(p, q)| p != q)?;
            let at = |p: &[u8]| format!("{:?}", &p[i * 4..i * 4 + 4]);
            Some(format!(
                "pixel ({}, {}) is {} natively and {} in the module",
                i % *w as usize,
                i / *w as usize,
                at(a),
                at(b)
            ))
        }
    }
}
