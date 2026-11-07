//! Serves the HTML example under `examples/web/` and the WebAssembly bundle that
//! `scripts/build-web.sh` writes to `target/web/`, on localhost only. A tool for trying Forager,
//! not a server to deploy: a thread per connection, GET only.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use forager_ports::ResourcePath;

const USAGE: &str = "usage: forager-dev-server [--port <port>]";

#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the dev server's address on stdout, its usage and errors on stderr, are its output"
)]
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
    // `cargo run` sets the manifest directory at run time too; it wins over the one compiled in,
    // which is stale when the binary was built at another path over the same `target/`.
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
    let root = manifest_dir.join("../..");
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
    /// Answers the requests of one connection, each in turn, until the client closes it or is
    /// idle for 10 seconds. The server never closes a connection while a response may be in
    /// flight: the Dev Container's port forwarding cuts a response short when the server's end
    /// closes before the browser has read it all (ERR_CONTENT_LENGTH_MISMATCH).
    fn answer(&self, stream: TcpStream) -> std::io::Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut reader = BufReader::new(&stream);
        loop {
            // The request line, then the headers up to the blank line, at most 64 KiB in all.
            let mut head = (&mut reader).take(64 * 1024);
            let mut request_line = String::new();
            if head.read_line(&mut request_line)? == 0 {
                return Ok(());
            }
            let mut close = false;
            let mut header = String::new();
            while head.read_line(&mut header)? > 0 && header.trim_end() != "" {
                close |= header.to_ascii_lowercase().replace(' ', "") == "connection:close\r\n";
                header.clear();
            }
            let mut parts = request_line.split_whitespace();
            let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let mut out = &stream;
            if method != "GET" {
                // A body it does not read would be taken for the next request: close after this.
                respond(
                    &mut out,
                    "405 Method Not Allowed",
                    "text/plain",
                    b"GET only\n",
                    true,
                )?;
                return linger(&stream);
            }
            match self
                .file(target)
                .and_then(|path| Some((std::fs::read(&path).ok()?, path)))
            {
                Some((body, path)) => {
                    respond(&mut out, "200 OK", content_type(&path), &body, close)?
                }
                None => respond(
                    &mut out,
                    "404 Not Found",
                    "text/plain",
                    b"not found\n",
                    close,
                )?,
            }
            if close {
                return linger(&stream);
            }
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

fn respond(
    stream: &mut impl Write,
    status: &str,
    kind: &str,
    body: &[u8],
    close: bool,
) -> std::io::Result<()> {
    let connection = if close { "close" } else { "keep-alive" };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: {connection}\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// Closes a connection gracefully: ends it with a FIN, then reads what the client still sends
/// until it closes, at most 5 seconds; a socket dropped with bytes unread is reset instead, and
/// the reset can overtake the response.
fn linger(stream: &TcpStream) -> std::io::Result<()> {
    stream.shutdown(Shutdown::Write)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut sink = [0; 4096];
    while matches!((&mut &*stream).read(&mut sink), Ok(n) if n > 0) {}
    Ok(())
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

    /// A site over a directory of its own holding `big.txt`, served on a port of its own for one
    /// connection; returns the address, the body, the directory, and the server's thread.
    fn serve_once(
        name: &str,
    ) -> (
        std::net::SocketAddr,
        Vec<u8>,
        PathBuf,
        std::thread::JoinHandle<()>,
    ) {
        let pages =
            std::env::temp_dir().join(format!("forager-dev-server-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&pages).unwrap();
        let body = vec![b'x'; 256 * 1024];
        std::fs::write(pages.join("big.txt"), &body).unwrap();
        let site = Site {
            pages: pages.clone(),
            bundle: pages.clone(),
        };
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            site.answer(stream).unwrap();
        });
        (address, body, pages, server)
    }

    /// Reads one response of `body`'s length from `client`, and returns its head.
    fn read_response(client: &mut TcpStream, body: &[u8]) -> String {
        let mut reader = BufReader::new(client);
        let mut head = String::new();
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
            head.push_str(&line);
            line.clear();
        }
        let mut received = vec![0; body.len()];
        reader.read_exact(&mut received).unwrap();
        assert_eq!(received, body);
        head
    }

    /// The server keeps a connection open after a response and answers the next request on
    /// it, each response whole; it is the client that closes. A server that closed first could
    /// have its response cut short by the Dev Container's port forwarding.
    #[test]
    fn a_connection_stays_open_for_the_next_request_until_the_client_closes_it() {
        let (address, body, pages, server) = serve_once("keep-alive");
        let mut client = TcpStream::connect(address).unwrap();
        for _ in 0..2 {
            client
                .write_all(b"GET /big.txt HTTP/1.1\r\nHost: x\r\n\r\n")
                .unwrap();
            let head = read_response(&mut client, &body);
            assert!(head.contains("Content-Length: 262144\r\n"), "{head}");
            assert!(head.contains("Connection: keep-alive\r\n"), "{head}");
        }
        drop(client);
        server.join().unwrap();
        std::fs::remove_dir_all(&pages).unwrap();
    }

    /// A request that asks to close gets its whole response, then the server's FIN, though the
    /// client sent bytes after it that the server never reads: a socket dropped with bytes unread
    /// would be reset, and the reset would discard what the client had not read yet.
    #[test]
    fn a_request_that_asks_to_close_gets_its_whole_response_and_then_the_end() {
        let (address, body, pages, server) = serve_once("close");
        let mut client = TcpStream::connect(address).unwrap();
        client
            .write_all(b"GET /big.txt HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .unwrap();
        // Bytes after the request that the server never reads, sent once it has read the request.
        std::thread::sleep(Duration::from_millis(100));
        client.write_all(&[b'y'; 1024]).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        drop(client);
        server.join().unwrap();
        std::fs::remove_dir_all(&pages).unwrap();
        let head = b"Connection: close\r\n";
        assert!(response.windows(head.len()).any(|w| w == head));
        assert!(
            response.ends_with(&body),
            "{} bytes received",
            response.len()
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
