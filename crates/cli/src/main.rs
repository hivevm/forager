//! The Forager command line, a host of the core: it owns the file system.

mod files;

use std::path::Path;
use std::process::ExitCode;

use forager_ports::ResourcePath;
use forager_style_ir::Diagnostic;

use crate::files::{FileLoader, block_on};

const USAGE: &str = "usage: forager read <mapfile> [--format yaml|json]";

enum Format {
    Yaml,
    Json,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("read") => match parse_read(&args[1..]) {
            Ok((mapfile, format)) => read(Path::new(mapfile), &format),
            Err(message) => usage(&message),
        },
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => usage(&format!("unknown command `{other}`")),
        None => usage("a command is required"),
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!("forager: {message}\n{USAGE}");
    ExitCode::from(2)
}

fn parse_read(args: &[String]) -> Result<(&str, Format), String> {
    let mut mapfile = None;
    let mut format = Format::Yaml;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--format" => {
                format = match iter.next().map(String::as_str) {
                    Some("yaml") => Format::Yaml,
                    Some("json") => Format::Json,
                    _ => return Err("--format takes yaml or json".to_owned()),
                }
            }
            _ if mapfile.is_none() => mapfile = Some(arg.as_str()),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    Ok((mapfile.ok_or("a mapfile is required")?, format))
}

/// Loads a mapfile with its directory as the base context and prints its style document.
fn read(mapfile: &Path, format: &Format) -> ExitCode {
    let (loader, name) = match FileLoader::for_mapfile(mapfile) {
        Ok(found) => found,
        Err(message) => {
            eprintln!("forager: {}: {message}", mapfile.display());
            return ExitCode::FAILURE;
        }
    };
    let path = match ResourcePath::new(&name) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("forager: {}: {e}", mapfile.display());
            return ExitCode::FAILURE;
        }
    };
    let loaded = block_on(forager_mapfile::load(&path, &loader));
    for diagnostic in &loaded.diagnostics {
        eprintln!("{diagnostic}");
    }
    let Some(document) = loaded.document else {
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

fn report(diagnostic: &Diagnostic) -> ExitCode {
    eprintln!("{diagnostic}");
    ExitCode::FAILURE
}
