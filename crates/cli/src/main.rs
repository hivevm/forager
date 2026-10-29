//! The Forager command line, a host of the core: it owns the file system.

mod files;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use forager_adapter_epsg::EpsgTable;
use forager_adapter_geojson::GeoJsonSource;
use forager_adapter_shapefile::ShapefileSource;
use forager_ports::{
    FeatureQuery, FeatureSource, ProjectionResolver, ResourceLoader, ResourcePath, SourceError,
};
use forager_render_agg::AggRenderer;
use forager_style_ir::{Diagnostic, StyleDocument};

use crate::files::{DataLoader, FileLoader};

const USAGE: &str =
    "usage: forager read <mapfile> [--format yaml|json] [--epsg <file>] [--base <dir>]
       forager render <mapfile> --source <layer>=<file.geojson>... -o <image.png> [--epsg <file>]
                      [--base <dir>]

--epsg names a table of EPSG definitions, lines `<code> +proj=... <>`; EPSG:4326, EPSG:3857,
and the WGS84 UTM zones are built in.
--base names the base context, a directory that holds the mapfile: INCLUDEs and other paths
resolve inside it, `..` included. By default it is the mapfile's directory.";

enum Format {
    Yaml,
    Json,
}

struct ReadArgs {
    mapfile: PathBuf,
    format: Format,
    epsg: Option<PathBuf>,
    base: Option<PathBuf>,
}

struct RenderArgs {
    mapfile: PathBuf,
    sources: Vec<(String, PathBuf)>,
    output: PathBuf,
    epsg: Option<PathBuf>,
    base: Option<PathBuf>,
}

fn main() -> ExitCode {
    // Arguments as the OS gives them: a path need not be UTF-8.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|a| a.to_str()) {
        Some("read") => match parse_read(&args[1..]) {
            Ok(read_args) => read(&read_args),
            Err(message) => usage(&message),
        },
        Some("render") => match parse_render(&args[1..]) {
            Ok(render_args) => render(&render_args),
            Err(message) => usage(&message),
        },
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => usage(&format!("unknown command `{other}`")),
        None if args.is_empty() => usage("a command is required"),
        None => usage("unknown command"),
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!("forager: {message}\n{USAGE}");
    ExitCode::from(2)
}

fn parse_read(args: &[OsString]) -> Result<ReadArgs, String> {
    let mut mapfile = None;
    let mut format = Format::Yaml;
    let mut epsg = None;
    let mut base = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--format") => {
                format = match iter.next().and_then(|a| a.to_str()) {
                    Some("yaml") => Format::Yaml,
                    Some("json") => Format::Json,
                    _ => return Err("--format takes yaml or json".to_owned()),
                }
            }
            Some("--epsg") => epsg = Some(PathBuf::from(iter.next().ok_or("--epsg takes a file")?)),
            Some("--base") => base = Some(PathBuf::from(iter.next().ok_or("--base takes a dir")?)),
            _ if mapfile.is_none() => mapfile = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{}`", arg.to_string_lossy())),
        }
    }
    Ok(ReadArgs {
        mapfile: mapfile.ok_or("a mapfile is required")?,
        format,
        epsg,
        base,
    })
}

fn parse_render(args: &[OsString]) -> Result<RenderArgs, String> {
    let mut mapfile = None;
    let mut sources = Vec::new();
    let mut output = None;
    let mut epsg = None;
    let mut base = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--source") => {
                let binding = iter.next().ok_or("--source takes <layer>=<file>")?;
                let text = binding.to_str().ok_or("--source must be valid UTF-8")?;
                let (layer, file) = text
                    .split_once('=')
                    .ok_or("--source takes <layer>=<file>")?;
                if sources
                    .iter()
                    .any(|(named, _): &(String, PathBuf)| named == layer)
                {
                    return Err(format!("--source names layer `{layer}` twice"));
                }
                sources.push((layer.to_owned(), PathBuf::from(file)));
            }
            Some("-o") => output = Some(PathBuf::from(iter.next().ok_or("-o takes a file")?)),
            Some("--epsg") => epsg = Some(PathBuf::from(iter.next().ok_or("--epsg takes a file")?)),
            Some("--base") => base = Some(PathBuf::from(iter.next().ok_or("--base takes a dir")?)),
            _ if mapfile.is_none() => mapfile = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{}`", arg.to_string_lossy())),
        }
    }
    Ok(RenderArgs {
        mapfile: mapfile.ok_or("a mapfile is required")?,
        sources,
        output: output.ok_or("-o <image.png> is required")?,
        epsg,
        base,
    })
}

/// Loads a mapfile inside `base`, by default its directory, resolving `init=` projections with the
/// built-in EPSG definitions and those of `epsg` (ADR-0023), and prints every diagnostic. The
/// loader of the base context comes with the document, for the files its layers read.
fn load(
    mapfile: &Path,
    epsg: Option<&Path>,
    base: Option<&Path>,
) -> Option<(StyleDocument, FileLoader)> {
    let resolver = match epsg {
        None => EpsgTable::new(),
        Some(file) => match read_epsg(file) {
            Ok(table) => table,
            Err(message) => {
                eprintln!("forager: {}:\n  {message}", file.display());
                return None;
            }
        },
    };
    let (loader, name) = FileLoader::for_mapfile(mapfile, base)
        .map_err(|message| eprintln!("forager: {}: {message}", mapfile.display()))
        .ok()?;
    let path = ResourcePath::new(&name)
        .map_err(|e| eprintln!("forager: {}: {e}", mapfile.display()))
        .ok()?;
    let document = load_document(&path, &loader, &resolver)
        .map_err(|diagnostic| eprintln!("{diagnostic}"))
        .ok()??;
    Some((document, loader))
}

/// Loads the mapfile at `path` through `loader` and prints the frontend's diagnostics; a loader
/// that waits is the `port-waited` error, since nothing here could wake it.
fn load_document(
    path: &ResourcePath,
    loader: &impl ResourceLoader,
    resolver: &impl ProjectionResolver,
) -> Result<Option<StyleDocument>, Diagnostic> {
    let loaded = now(forager_mapfile::load(path, loader, Some(resolver)))?;
    for diagnostic in &loaded.diagnostics {
        eprintln!("{diagnostic}");
    }
    Ok(loaded.document)
}

/// Runs the frontend or the engine in one poll (`forager_ports::poll_once`): the CLI's ports
/// read local files and never wait. A future that waits is the `port-waited` error the browser
/// bindings report, rather than a command that hangs.
fn now<F: Future>(future: F) -> Result<F::Output, Diagnostic> {
    forager_ports::poll_once(future).ok_or_else(|| {
        Diagnostic::error("port-waited", "a port waited, which the CLI cannot", None)
    })
}

/// Prints a mapfile's style document.
fn read(args: &ReadArgs) -> ExitCode {
    let Some((document, _)) = load(&args.mapfile, args.epsg.as_deref(), args.base.as_deref())
    else {
        return ExitCode::FAILURE;
    };
    let text = match args.format {
        Format::Yaml => document.to_yaml(),
        Format::Json => document.to_json().map(|json| json + "\n"),
    };
    match text {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(diagnostic) => report(&diagnostic),
    }
}

/// Renders a mapfile with GeoJSON files bound to its layers by name, and writes a PNG.
fn render(args: &RenderArgs) -> ExitCode {
    let Some((document, loader)) = load(&args.mapfile, args.epsg.as_deref(), args.base.as_deref())
    else {
        return ExitCode::FAILURE;
    };
    let data = loader.for_data(MAX_SOURCE_BYTES);
    let mut shapefiles = Shapefiles::new(&data);
    let mut diagnostics = Vec::new();
    let sources = forager_engine::bind_sources(
        &document.map,
        held_sources(&document, &args.sources, &data),
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
    for diagnostic in &diagnostics {
        eprintln!("{diagnostic}");
    }
    let rendered = match now(forager_engine::render(
        &document.map,
        &sources,
        AggRenderer::new(),
    )) {
        Ok(rendered) => rendered,
        Err(diagnostic) => return report(&diagnostic),
    };
    for diagnostic in &rendered.diagnostics {
        eprintln!("{diagnostic}");
    }
    let Some(image) = rendered.image else {
        return ExitCode::FAILURE;
    };
    let written = forager_encode::png(&image)
        .and_then(|png| std::fs::write(&args.output, png).map_err(|e| e.to_string()));
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("forager: {}: {message}", args.output.display());
            ExitCode::FAILURE
        }
    }
}

fn report(diagnostic: &Diagnostic) -> ExitCode {
    eprintln!("{diagnostic}");
    ExitCode::FAILURE
}

/// The largest data file the CLI reads, a GeoJSON file or one of a shapefile's files (Q-9).
const MAX_SOURCE_BYTES: u64 = 256 * 1024 * 1024;

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
/// report. A layer whose features the engine does not read, one OFF or a raster, is not bound to
/// its `DATA`: its files would be read into memory for nothing, and a raster's `DATA` is no
/// shapefile.
fn held_sources(
    document: &StyleDocument,
    named: &[(String, PathBuf)],
    data: &DataLoader,
) -> Vec<(String, Held)> {
    let mut held: BTreeMap<String, Held> = BTreeMap::new();
    for layer in &document.map.layers {
        let (Some(name), Some(source)) = (&layer.name, &layer.source) else {
            continue;
        };
        if forager_engine::reads_features(layer)
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

/// The EPSG table a host file supplies (ADR-0023), within the size limit of a source. Each
/// unusable line is a line of the message.
fn read_epsg(file: &Path) -> Result<EpsgTable, String> {
    EpsgTable::with_table(&read_bounded(file)?).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n  ")
    })
}

/// A text file of at most [`MAX_SOURCE_BYTES`] (Q-9). The limit bounds what is read, not only
/// what the file system reports, so that a device or a pipe cannot exhaust memory.
fn read_bounded(file: &Path) -> Result<String, String> {
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

    /// A loader that waits once on its first load, as one that fetches over a network would.
    struct WaitingLoader(Cell<bool>);

    impl ResourceLoader for WaitingLoader {
        async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
            std::future::poll_fn(|_| {
                if self.0.replace(true) {
                    std::task::Poll::Ready(())
                } else {
                    std::task::Poll::Pending
                }
            })
            .await;
            Ok(b"MAP END".to_vec())
        }
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

    #[test]
    fn a_loader_that_waits_fails_the_load_instead_of_hanging() {
        let loader = WaitingLoader(Cell::new(false));
        let path = ResourcePath::new("m.map").unwrap();
        let error = load_document(&path, &loader, &EpsgTable::new()).unwrap_err();
        assert_eq!(error.code, "port-waited");
        assert!(loader.0.get(), "the loader was polled");
    }
}
