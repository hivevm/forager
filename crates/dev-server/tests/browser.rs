//! The browser example in a real browser (G-14, ADR-0040): the dev
//! server serves `examples/web/` and the bundle `scripts/build-web.sh` wrote, headless Chrome
//! opens the page over WebDriver, and the canvas and the list of diagnostics are compared with
//! the native render of the same input.
//!
//! The cases run only where `FORAGER_BROWSER_TEST=required`, as in the browser job of
//! `.github/workflows/checks.yml`; there a missing ChromeDriver, Chrome, or bundle fails them.
//! Elsewhere, as in the Dev Container, which has no browser (ADR-0009), they return at once.

// The client, beside the test in a directory of its own, which Cargo does not take for a test.
#[path = "browser/webdriver.rs"]
mod webdriver;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use forager_bindings_wasm::{Output, Viewport, render_map_with_files};
use serde_json::{Value, json};
use webdriver::{Driver, Process, Session};

/// This crate's directory: the run time's `CARGO_MANIFEST_DIR`, else the compiled-in path.
fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

/// How long the page may take to load the bundle, the sample, and the font, and to render.
const PAGE_RENDER: Duration = Duration::from_secs(120);

/// The repository's root, whose `examples/web/` and `target/web/` the dev server serves.
fn root() -> PathBuf {
    manifest_dir().join("../..")
}

/// Whether the cases run: only where the variable requires them.
#[expect(
    clippy::print_stderr,
    reason = "a case that does not run says so, so that a local pass is not read as a browser's"
)]
fn required(case: &str) -> bool {
    let required = std::env::var("FORAGER_BROWSER_TEST").is_ok_and(|v| v == "required");
    if !required {
        eprintln!(
            "browser test `{case}` not run: it runs where FORAGER_BROWSER_TEST=required, as in \
             the browser job of CI"
        );
    }
    required
}

/// The page served and opened in headless Chrome, with the processes that serve and show it.
struct Page {
    session: Session,
    _driver: Driver,
    _server: Process,
}

impl Page {
    /// Starts the dev server Cargo built for this test and ChromeDriver, each on a free port,
    /// opens the page, and waits until it has rendered its sample.
    fn open() -> Self {
        assert!(
            root().join("target/web/forager_bindings_wasm.js").is_file(),
            "no bundle in target/web; run `bash scripts/build-web.sh` first"
        );
        let port = webdriver::free_port();
        let mut server = Command::new(env!("CARGO_BIN_EXE_forager-dev-server"))
            .args(["--port", &port.to_string()])
            .env("CARGO_MANIFEST_DIR", manifest_dir())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the dev server starts");
        // It prints its address once it listens.
        let mut line = String::new();
        BufReader::new(server.stdout.take().expect("piped"))
            .read_line(&mut line)
            .expect("the dev server reports its address");
        assert!(line.starts_with("Serving"), "{line}");
        let server = Process(server);
        let binary = webdriver::chromedriver()
            .expect("no ChromeDriver in the directory CHROMEWEBDRIVER names, nor on PATH");
        let driver = Driver::start(&binary).unwrap();
        let session = driver.session().unwrap();
        session
            .navigate(&format!("http://127.0.0.1:{port}/"))
            .unwrap();
        let page = Self {
            session,
            _driver: driver,
            _server: server,
        };
        page.wait_for_render();
        page
    }

    fn run(&self, script: &str, args: &[Value]) -> Value {
        self.session.execute(script, args).unwrap()
    }

    /// Waits until the status line reports a render, the image's size or that none was drawn,
    /// and returns it. A status that reports a failure, of the render or of the bundle, fails at
    /// once, and any other after the time budget; each with the status it shows.
    fn wait_for_render(&self) -> String {
        let deadline = Instant::now() + PAGE_RENDER;
        loop {
            let status = self.run("return document.getElementById('status').textContent;", &[]);
            let status = status.as_str().unwrap_or_default().to_owned();
            if status.contains(" px in ") || status.starts_with("The map could not be drawn") {
                return status;
            }
            assert!(
                !status.starts_with("Rendering failed")
                    && !status.starts_with("The WebAssembly bundle did not load"),
                "the page failed: {status}"
            );
            assert!(
                Instant::now() < deadline,
                "the page did not render in {PAGE_RENDER:?}; its status: {status}"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// The canvas `#map`: its width, its height, and its RGBA bytes as `getImageData` reads them.
    fn canvas(&self) -> (u32, u32, Vec<u8>) {
        let image = self.run(
            "const canvas = document.getElementById('map');
             const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height);
             return [canvas.width, canvas.height, Array.from(data.data)];",
            &[],
        );
        let number = |v: &Value| u32::try_from(v.as_u64().unwrap()).unwrap();
        let bytes = image[2].as_array().unwrap();
        let bytes = bytes
            .iter()
            .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
            .collect();
        (number(&image[0]), number(&image[1]), bytes)
    }

    /// The entries of `#diagnostics`, each its class and its text.
    fn diagnostics(&self) -> Vec<(String, String)> {
        let items = self.run(
            "return Array.from(document.querySelectorAll('#diagnostics li'),
                               item => [item.className, item.textContent]);",
            &[],
        );
        serde_json::from_value(items).unwrap()
    }
}

/// The page's sample: its mapfile and the GeoJSON it binds to the layers by name.
fn sample() -> (String, String) {
    let web = root().join("examples/web");
    let read = |name: &str| std::fs::read_to_string(web.join(name)).unwrap();
    let mut sources = serde_json::Map::new();
    for layer in ["landuse", "rivers", "buildings", "roads", "places"] {
        sources.insert(layer.to_owned(), read(&format!("{layer}.geojson")).into());
    }
    (read("sample.map"), Value::Object(sources).to_string())
}

/// The files the page hands to every render, its `FILES`, by their paths.
fn page_files() -> BTreeMap<String, Vec<u8>> {
    let web = root().join("examples/web");
    ["fonts.lst", "vendor/dejavu/DejaVuSans.ttf"]
        .into_iter()
        .map(|path| (path.to_owned(), std::fs::read(web.join(path)).unwrap()))
        .collect()
}

/// The native render of `mapfile` with the sample's sources and the page's files, at the page's
/// viewport: the mapfile's own SIZE and EXTENT.
fn native(mapfile: &str) -> Output {
    let (_, sources) = sample();
    render_map_with_files(mapfile, &sources, None, &Viewport::default(), &page_files())
}

/// The entries the page lists for `output`'s diagnostics, as its `show` writes them: the class is
/// the severity, the text the span, the severity, the code, and the message.
fn listed(output: &Output) -> Vec<(String, String)> {
    output
        .diagnostics
        .iter()
        .map(|d| {
            let severity = serde_json::to_value(d.severity).unwrap();
            let severity = severity.as_str().unwrap().to_owned();
            let span = d
                .span
                .as_ref()
                .map(|s| format!("{s}: "))
                .unwrap_or_default();
            let text = format!("{span}{severity} [{}]: {}", d.code, d.message);
            (severity, text)
        })
        .collect()
}

/// The index of the first pixel at which `a` and `b` differ, if they do.
fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    a.chunks(4).zip(b.chunks(4)).position(|(p, q)| p != q)
}

#[test]
fn the_page_shows_the_image_the_bindings_return_for_its_sample() {
    // Verifies: G-14, ADR-0040
    if !required("the_page_shows_the_image_the_bindings_return_for_its_sample") {
        return;
    }
    let page = Page::open();
    let (mapfile, _) = sample();
    let reference = native(&mapfile);
    let image = reference.image.as_ref().expect("the sample renders");
    // The canvas keeps its pixels premultiplied, so that only an opaque pixel reads back as it was
    // put; the sample's IMAGECOLOR is opaque, and it sets no TRANSPARENT.
    let (width, height, pixels) = page.canvas();
    assert_eq!((width, height), (image.width, image.height));
    assert_eq!(pixels.len(), image.pixels.len());
    let differs = first_difference(&pixels, &image.pixels);
    assert!(
        differs.is_none(),
        "the canvas differs from the native render first at pixel {differs:?} of {width} × {height}"
    );
    // The sample renders clean, natively and on the page.
    assert_eq!(page.diagnostics(), listed(&reference));
}

/// A mapfile with known faults, which still renders: a layer whose source carries credentials,
/// which the bindings drop with an error (Q-7), a layer the page binds no GeoJSON to, and four of
/// the five layers it binds GeoJSON to not named. Its diagnostics are an error and warnings, some
/// with a span and some without.
const FAULTY: &str = r##"MAP
  SIZE 200 100
  EXTENT 0 0 4096 2048
  IMAGECOLOR "#ffffff"
  LAYER
    NAME "roads"
    TYPE LINE
    STATUS ON
    CLASS
      STYLE
        COLOR 10 20 30
        WIDTH 2
      END
    END
  END
  LAYER
    NAME "nowhere"
    TYPE POINT
    STATUS ON
  END
  LAYER
    NAME "secret"
    TYPE POLYGON
    STATUS ON
    CONNECTIONTYPE POSTGIS
    CONNECTION "host=db user=u password=secret"
    DATA "geom from t"
  END
END
"##;

#[test]
fn the_page_lists_each_diagnostic_of_a_faulty_mapfile() {
    // Verifies: ADR-0040
    if !required("the_page_lists_each_diagnostic_of_a_faulty_mapfile") {
        return;
    }
    let page = Page::open();
    let reference = listed(&native(FAULTY));
    // The list holds both severities, so that the page's classes of both are compared.
    for severity in ["error", "warning"] {
        assert!(
            reference.iter().any(|(class, _)| class == severity),
            "the faulty mapfile gives no {severity}: {reference:?}"
        );
    }
    // The mapfile typed into its field, and Render pressed; the status is cleared first, so that
    // the wait sees this render's.
    page.run(
        "document.getElementById('mapfile').value = arguments[0];
         document.getElementById('status').textContent = '';
         document.getElementById('render').click();",
        &[json!(FAULTY)],
    );
    page.wait_for_render();
    assert_eq!(page.diagnostics(), reference);
}

/// The client against a stand-in for ChromeDriver on localhost, without a browser, so that it is
/// tested where the cases above do not run: an answer is read to its Content-Length, and an
/// answer of the protocol's errors is returned as an error with its code and message.
#[test]
fn the_webdriver_client_reads_answers_and_the_protocol_s_errors() {
    // Verifies: ADR-0040
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let answers = [
        {
            let body = r#"{"value": {"ready": true}}"#;
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
        },
        {
            let body = r#"{"value": {"error": "no such window", "message": "gone"}}"#;
            format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
        },
    ];
    let stand_in = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for answer in answers {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            // The head, then the body of the length it announces.
            let whole = |request: &[u8]| {
                let text = String::from_utf8_lossy(request);
                let Some((head, body)) = text.split_once("\r\n\r\n") else {
                    return false;
                };
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .map_or(0, |n| n.parse().unwrap());
                body.len() == length
            };
            while !whole(&request) {
                let n = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..n]);
            }
            stream.write_all(answer.as_bytes()).unwrap();
            requests.push(String::from_utf8(request).unwrap());
        }
        requests
    });
    let ready = webdriver::request(address, "GET", "/status", None);
    assert_eq!(ready, Ok(json!({"ready": true})));
    let gone = webdriver::request(
        address,
        "POST",
        "/session/x/url",
        Some(&json!({"url": "/"})),
    );
    assert_eq!(
        gone,
        Err(r#"POST /session/x/url: HTTP 404, "no such window": "gone""#.to_owned())
    );
    let requests = stand_in.join().unwrap();
    assert!(
        requests[0].starts_with("GET /status HTTP/1.1\r\n"),
        "{requests:?}"
    );
    assert!(
        requests[1].contains("Content-Length: 11\r\n"),
        "{requests:?}"
    );
    assert!(
        requests[1].ends_with("\r\n\r\n{\"url\":\"/\"}"),
        "{requests:?}"
    );
}
