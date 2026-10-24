//! Serves the HTML example under `examples/web/` and the WebAssembly bundle that
//! `scripts/build-web.sh` writes to `target/web/`, on localhost only. A tool for trying Forager,
//! not a server to deploy: one request at a time, GET only.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use forager_ports::ResourcePath;

const USAGE: &str = "usage: forager-dev-server [--port <port>]";

fn main() {
    let mut args = std::env::args().skip(1);
    let mut port: u16 = 8000;
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next().and_then(|p| p.parse().ok())) {
            ("--port", Some(p)) => port = p,
            _ => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let site = Site {
        pages: root.join("examples/web"),
        bundle: root.join("target/web"),
    };
    if !site.bundle.join("forager_bindings_wasm.js").exists() {
        eprintln!(
            "forager-dev-server: no bundle in target/web; run `bash scripts/build-web.sh` first"
        );
    }
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("forager-dev-server: cannot listen on 127.0.0.1:{port}: {e}");
            std::process::exit(1);
        }
    };
    println!("Serving the Forager example on http://127.0.0.1:{port}/");
    // A thread per connection: browsers open idle connections in advance, and one of them must not
    // hold up the others.
    let site = Arc::new(site);
    for stream in listener.incoming().flatten() {
        let site = Arc::clone(&site);
        std::thread::spawn(move || {
            if let Err(e) = site.answer(stream) {
                eprintln!("forager-dev-server: {e}");
            }
        });
    }
}

struct Site {
    pages: PathBuf,
    bundle: PathBuf,
}

impl Site {
    fn answer(&self, mut stream: TcpStream) -> std::io::Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        // The request line, then the headers up to the blank line, at most 64 KiB in all: reading
        // them all lets the connection close cleanly after the response.
        let mut reader = BufReader::new((&stream).take(64 * 1024));
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;
        let mut header = String::new();
        while reader.read_line(&mut header)? > 0 && header.trim_end() != "" {
            header.clear();
        }
        let mut parts = request_line.split_whitespace();
        let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        if method != "GET" {
            return respond(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain",
                b"GET only\n",
            );
        }
        match self.file(target) {
            Some(path) => match std::fs::read(&path) {
                Ok(body) => respond(&mut stream, "200 OK", content_type(&path), &body),
                Err(_) => respond(&mut stream, "404 Not Found", "text/plain", b"not found\n"),
            },
            None => respond(&mut stream, "404 Not Found", "text/plain", b"not found\n"),
        }
    }

    /// The file a request target names, inside the example or the bundle; a path that would
    /// leave them is refused by `ResourcePath`.
    fn file(&self, target: &str) -> Option<PathBuf> {
        let path = target.split(['?', '#']).next().unwrap_or("");
        let path = if path == "/" {
            "index.html"
        } else {
            path.trim_start_matches('/')
        };
        let resource = ResourcePath::new(path).ok()?;
        match resource.as_str().strip_prefix("pkg/") {
            Some(bundled) => Some(self.bundle.join(bundled)),
            None => Some(self.pages.join(resource.as_str())),
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json" | "geojson") => "application/json",
        Some("css") => "text/css; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> Site {
        Site {
            pages: PathBuf::from("/pages"),
            bundle: PathBuf::from("/bundle"),
        }
    }

    #[test]
    fn requests_map_to_the_example_and_the_bundle() {
        let site = site();
        assert_eq!(site.file("/"), Some(PathBuf::from("/pages/index.html")));
        assert_eq!(
            site.file("/sample.map?x=1"),
            Some(PathBuf::from("/pages/sample.map"))
        );
        assert_eq!(
            site.file("/pkg/forager_bindings_wasm.js"),
            Some(PathBuf::from("/bundle/forager_bindings_wasm.js"))
        );
    }

    #[test]
    fn paths_out_of_the_example_are_refused() {
        // Verifies: Q-8
        let site = site();
        assert_eq!(site.file("/../Cargo.toml"), None);
        assert_eq!(site.file("/pkg/../../secret"), None);
        assert_eq!(
            site.file("//etc/passwd").map(|p| p.starts_with("/pages")),
            Some(true)
        );
    }
}
