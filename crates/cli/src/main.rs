//! The Forager command line, a host of the core: it owns the file system.

mod files;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use forager_adapter_geojson::GeoJsonSource;
use forager_ports::ResourcePath;
use forager_render_agg::AggRenderer;
use forager_style_ir::{Diagnostic, StyleDocument};

use crate::files::{FileLoader, block_on};

const USAGE: &str = "usage: forager read <mapfile> [--format yaml|json]
       forager render <mapfile> --source <layer>=<file.geojson>... -o <image.png>";

enum Format {
    Yaml,
    Json,
}

struct RenderArgs {
    mapfile: PathBuf,
    sources: Vec<(String, PathBuf)>,
    output: PathBuf,
}

fn main() -> ExitCode {
    // Arguments as the OS gives them: a path need not be UTF-8.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|a| a.to_str()) {
        Some("read") => match parse_read(&args[1..]) {
            Ok((mapfile, format)) => read(Path::new(mapfile), &format),
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

fn parse_read(args: &[OsString]) -> Result<(&OsString, Format), String> {
    let mut mapfile = None;
    let mut format = Format::Yaml;
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
            _ if mapfile.is_none() => mapfile = Some(arg),
            _ => return Err(format!("unexpected argument `{}`", arg.to_string_lossy())),
        }
    }
    Ok((mapfile.ok_or("a mapfile is required")?, format))
}

fn parse_render(args: &[OsString]) -> Result<RenderArgs, String> {
    let mut mapfile = None;
    let mut sources = Vec::new();
    let mut output = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--source") => {
                let binding = iter.next().ok_or("--source takes <layer>=<file>")?;
                let text = binding.to_str().ok_or("--source must be valid UTF-8")?;
                let (layer, file) = text
                    .split_once('=')
                    .ok_or("--source takes <layer>=<file>")?;
                sources.push((layer.to_owned(), PathBuf::from(file)));
            }
            Some("-o") => output = Some(PathBuf::from(iter.next().ok_or("-o takes a file")?)),
            _ if mapfile.is_none() => mapfile = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{}`", arg.to_string_lossy())),
        }
    }
    Ok(RenderArgs {
        mapfile: mapfile.ok_or("a mapfile is required")?,
        sources,
        output: output.ok_or("-o <image.png> is required")?,
    })
}

/// Loads a mapfile with its directory as the base context, printing every diagnostic.
fn load(mapfile: &Path) -> Option<StyleDocument> {
    let (loader, name) = FileLoader::for_mapfile(mapfile)
        .map_err(|message| eprintln!("forager: {}: {message}", mapfile.display()))
        .ok()?;
    let path = ResourcePath::new(&name)
        .map_err(|e| eprintln!("forager: {}: {e}", mapfile.display()))
        .ok()?;
    let loaded = block_on(forager_mapfile::load(&path, &loader));
    for diagnostic in &loaded.diagnostics {
        eprintln!("{diagnostic}");
    }
    loaded.document
}

/// Prints a mapfile's style document.
fn read(mapfile: &Path, format: &Format) -> ExitCode {
    let Some(document) = load(mapfile) else {
        return ExitCode::FAILURE;
    };
    let text = match format {
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
    let Some(document) = load(&args.mapfile) else {
        return ExitCode::FAILURE;
    };
    let mut sources = BTreeMap::new();
    for (layer, file) in &args.sources {
        let parsed = std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|text| GeoJsonSource::parse(&text).map_err(|e| e.message));
        match parsed {
            Ok(source) => {
                sources.insert(layer.clone(), source);
            }
            Err(message) => {
                eprintln!("forager: {}: {message}", file.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let rendered = block_on(forager_engine::render(
        &document.map,
        &sources,
        AggRenderer::new(),
    ));
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
