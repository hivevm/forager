//! The binding of the Forager command line, a host of the core: how `forager render` binds the
//! layers of a map to their sources and reads the fonts of its labels, inside a base context.
//! The binary is built on it, and the conformance harness renders through it, with the suite's
//! directory as the base context.

mod files;

pub use crate::files::FileLoader;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use forager_adapter_geojson::GeoJsonSource;
use forager_adapter_shapefile::ShapefileSource;
use forager_engine::Rendered;
use forager_ports::{FeatureQuery, FeatureSource, ResourceLoader, ResourcePath, SourceError};
use forager_render_agg::AggRenderer;
use forager_style_ir::{Diagnostic, Map};

use crate::files::DataLoader;

/// The largest data file the CLI reads, a GeoJSON file or one of a shapefile's files (Q-9).
const MAX_SOURCE_BYTES: u64 = 256 * 1024 * 1024;

/// Renders `map` with the GeoJSON files `named` binds to its layers by name and the shapefiles
/// its local layers name in `base`, reading the fonts of its labels from `base` too, each file
/// bounded by its limit (Q-9). The rendering's diagnostics are the binding's, then the engine's;
/// a read that waited (see [`now`]) is the `port-waited` error among them, with no image.
pub fn render(map: &Map, named: &[(String, PathBuf)], base: &FileLoader) -> Rendered {
    // The fonts of the labels come from the same base context, each bounded by the engine's
    // limit for a font, read no further than that (Q-9).
    let fonts = base.clone().for_data(forager_engine::MAX_FONT_BYTES as u64);
    let data = base.clone().for_data(MAX_SOURCE_BYTES);
    let mut shapefiles = Shapefiles::new(&data);
    let mut diagnostics = Vec::new();
    let sources = forager_engine::bind_sources(
        map,
        held_sources(map, named, &data),
        |held| match held {
            Held::GeoJson(file) => read_source(&file)
                .map(Source::GeoJson)
                .map_err(|message| format!("{}: {message}", file.display())),
            Held::Shapefile(path) => shapefiles
                .open(&path)
                .map(Source::Shapefile)
                .map_err(|message| format!("{path}: {message}")),
        },
        &mut diagnostics,
    );
    let mut rendered = now(forager_engine::render_with_files(
        map,
        &sources,
        &fonts,
        AggRenderer::new(),
    ))
    .unwrap_or_else(|waited| Rendered {
        image: None,
        diagnostics: vec![waited],
        labels: Vec::new(),
    });
    diagnostics.append(&mut rendered.diagnostics);
    rendered.diagnostics = diagnostics;
    rendered
}

/// Runs the frontend or the engine in one poll (`forager_ports::poll_once`): the CLI's ports
/// read local files and never wait. A future that waits is the `port-waited` error the browser
/// bindings report, rather than a command that hangs.
///
/// # Errors
///
/// The `port-waited` error, when the future waited.
pub fn now<F: Future>(future: F) -> Result<F::Output, Diagnostic> {
    forager_ports::poll_once(future).ok_or_else(|| {
        Diagnostic::error("port-waited", "a port waited, which the CLI cannot", None)
    })
}

/// What the CLI holds for a layer before it reads it.
enum Held {
    /// A GeoJSON file `--source` names.
    GeoJson(PathBuf),
    /// A shapefile a local layer's `DATA` names, inside the base context.
    Shapefile(String),
}

/// Each local layer's shapefile, by its `DATA`, and the GeoJSON files `--source` names, which win
/// for their layer. A layer whose `.shp` the base context does not hold is left unbound, so the
/// engine skips it as any layer without a source, and the CLI reports what the browser bindings
/// report. A layer whose bound source the engine does not read, one OFF, a raster, or one with
/// inline features, is not bound to its `DATA`: its files would be read into memory for nothing,
/// and a raster's `DATA` is no shapefile.
fn held_sources(map: &Map, named: &[(String, PathBuf)], data: &DataLoader) -> Vec<(String, Held)> {
    let mut held: BTreeMap<String, Held> = BTreeMap::new();
    for layer in &map.layers {
        let (Some(name), Some(source)) = (&layer.name, &layer.source) else {
            continue;
        };
        if forager_engine::reads_bound_source(layer)
            && source.kind == "local"
            && let Some(path) = source.props.get("data")
            && shapefile_exists(data, path)
        {
            held.insert(name.clone(), Held::Shapefile(path.clone()));
        }
    }
    for (layer, file) in named {
        held.insert(layer.clone(), Held::GeoJson(file.clone()));
    }
    held.into_iter().collect()
}

/// Whether the base context holds the `.shp` or `.SHP` of `data`, whose extension, if any, the
/// shapefile adapter replaces as `msShapefileOpen` does.
fn shapefile_exists(loader: &DataLoader, data: &str) -> bool {
    let base = forager_adapter_shapefile::basename(data);
    ["shp", "SHP"].iter().any(|extension| {
        ResourcePath::new(&format!("{base}.{extension}")).is_ok_and(|path| loader.holds(&path))
    })
}

/// The shapefiles the CLI opened, by the name the adapter cuts from a layer's `DATA`, so that the
/// layers naming one shapefile share it and its files are read once (ADR-0024: a layer costs
/// memory in proportion to its files, not to the layers that name them). A shapefile that failed
/// to open is not tried again.
struct Shapefiles<'a, L> {
    loader: &'a L,
    opened: BTreeMap<String, Result<Rc<ShapefileSource>, String>>,
}

impl<'a, L: ResourceLoader> Shapefiles<'a, L> {
    fn new(loader: &'a L) -> Self {
        Self {
            loader,
            opened: BTreeMap::new(),
        }
    }

    /// The shapefile at `data`, a path the frontend resolved inside the base context.
    fn open(&mut self, data: &str) -> Result<Rc<ShapefileSource>, String> {
        let name = forager_adapter_shapefile::basename(data).to_owned();
        let loader = self.loader;
        self.opened
            .entry(name)
            .or_insert_with(|| open_shapefile(loader, data).map(Rc::new))
            .clone()
    }
}

fn open_shapefile(loader: &impl ResourceLoader, data: &str) -> Result<ShapefileSource, String> {
    let path = ResourcePath::new(data).map_err(|e| e.to_string())?;
    match forager_ports::poll_once(ShapefileSource::open(loader, &path)) {
        Some(opened) => opened.map_err(|e| e.message),
        None => Err("its loader waited".to_owned()),
    }
}

/// A layer's feature source, of whichever adapter the CLI bound to it; layers that name one
/// shapefile share it.
enum Source {
    GeoJson(GeoJsonSource),
    Shapefile(Rc<ShapefileSource>),
}

impl FeatureSource for Source {
    async fn features(&self, query: &FeatureQuery) -> Result<forager_ports::Features, SourceError> {
        match self {
            Self::GeoJson(source) => source.features(query).await,
            Self::Shapefile(source) => source.features(query).await,
        }
    }
}

fn read_source(file: &Path) -> Result<GeoJsonSource, String> {
    GeoJsonSource::parse(&read_bounded(file)?).map_err(|e| e.message)
}

/// A text file of at most the limit of a data source (Q-9). The limit bounds what is read, not only
/// what the file system reports, so that a device or a pipe cannot exhaust memory.
///
/// # Errors
///
/// A file that cannot be read, that exceeds the limit, or that is not UTF-8 text.
pub fn read_bounded(file: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .and_then(|f| f.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(format!(
            "the file exceeds the limit of {MAX_SOURCE_BYTES} bytes"
        ));
    }
    String::from_utf8(bytes).map_err(|_| "the file is not UTF-8 text".to_owned())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use forager_ports::SourceError;

    use super::*;

    #[test]
    fn the_fonts_of_a_label_are_read_from_the_base_context_within_the_font_limit() {
        // The conformance harness hands `render` the suite as the base context; the fonts of a
        // case's labels are then read here, as `forager render` reads them: from the base context,
        // each refused by the CLI's loader past the engine's limit for a font (Q-9).
        let dir = std::env::temp_dir().join(format!("forager-lib-fonts-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("fonts")).unwrap();
        // A test font of the label crate, a subset of DejaVu Sans whose licence lies beside it.
        let sans = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
        std::fs::copy(sans, dir.join("fonts/sans.ttf")).unwrap();
        // One byte past the limit, sparse on disk.
        std::fs::File::create(dir.join("fonts/huge.ttf"))
            .and_then(|f| f.set_len(forager_engine::MAX_FONT_BYTES as u64 + 1))
            .unwrap();
        std::fs::write(
            dir.join("fonts.lst"),
            "sans fonts/sans.ttf\nhuge fonts/huge.ttf\n",
        )
        .unwrap();
        let rendered = |font: &str| {
            std::fs::write(
                dir.join("m.map"),
                format!(
                    "MAP SIZE 10 10 EXTENT 0 0 10 10 FONTSET 'fonts.lst'\n\
                     LAYER NAME 'p' TYPE POINT STATUS ON FEATURE POINTS 5 5 END END\n\
                     CLASS LABEL FONT '{font}' TEXT 'x' END END END END\n"
                ),
            )
            .unwrap();
            let (base, name) = FileLoader::for_mapfile(&dir.join("m.map"), None).unwrap();
            let path = ResourcePath::new(&name).unwrap();
            let loaded = now(forager_mapfile::load(&path, &base, None)).unwrap();
            let map = loaded.document.expect("the mapfile loads").map;
            render(&map, &[], &base)
        };
        let sans = rendered("sans");
        let huge = rendered("huge");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(sans.diagnostics.is_empty(), "{:?}", sans.diagnostics);
        assert_eq!(huge.diagnostics.len(), 1, "{:?}", huge.diagnostics);
        assert!(sans.image.is_some(), "{:?}", sans.diagnostics);
        assert_eq!(sans.labels.len(), 1, "{:?}", sans.diagnostics);
        // The CLI's loader refuses the font, with its own reason, before the engine sees it.
        let limit = format!(
            "the file exceeds the limit of {} bytes",
            forager_engine::MAX_FONT_BYTES
        );
        assert!(
            huge.diagnostics
                .iter()
                .any(|d| d.code == "label-font-unusable" && d.message.contains(&limit)),
            "{:?}",
            huge.diagnostics
        );
    }

    /// The files of one empty shapefile, loaded from memory, each load counted.
    struct CountingLoader {
        files: BTreeMap<&'static str, Vec<u8>>,
        loads: Cell<usize>,
    }

    impl CountingLoader {
        fn new() -> Self {
            let mut shx = vec![0u8; 100];
            shx[..4].copy_from_slice(&[0, 0, 0x27, 0x0a]);
            shx[24..28].copy_from_slice(&50i32.to_be_bytes());
            let mut dbf = vec![0u8; 33];
            dbf[8] = 33;
            let files = [
                ("poly.shp", vec![0u8; 100]),
                ("poly.shx", shx),
                ("poly.dbf", dbf),
                ("broken.shp", vec![0u8; 10]),
            ];
            Self {
                files: files.into_iter().collect(),
                loads: Cell::new(0),
            }
        }
    }

    impl ResourceLoader for CountingLoader {
        async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
            self.loads.set(self.loads.get() + 1);
            self.files
                .get(path.as_str())
                .cloned()
                .ok_or_else(|| SourceError::new("no such file"))
        }
    }

    #[test]
    fn layers_that_name_one_shapefile_read_its_files_once() {
        // Verifies: Q-9
        // ADR-0024: a layer costs memory in proportion to its files, so a shapefile that several
        // layers name is read, and held, once.
        let loader = CountingLoader::new();
        let mut shapefiles = Shapefiles::new(&loader);
        let first = shapefiles.open("poly").unwrap();
        assert_eq!(loader.loads.get(), 3, "the .shp, .shx, and .dbf");
        let second = shapefiles.open("poly.shp").unwrap();
        assert_eq!(loader.loads.get(), 3, "nothing is read again");
        assert!(Rc::ptr_eq(&first, &second), "the layers share one source");
        // A shapefile that fails to open is not read again either.
        assert!(shapefiles.open("broken").is_err());
        let loads = loader.loads.get();
        assert!(shapefiles.open("broken.shp").is_err());
        assert_eq!(loader.loads.get(), loads);
    }
}
