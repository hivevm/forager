//! The browser test's WebDriver client, written on `std` and `serde_json` alone
//! (ADR-draft-browser-test-over-webdriver): ChromeDriver started on a free port of localhost, and
//! the four calls of the W3C protocol the test makes, New Session, Navigate To, Execute Script,
//! and Delete Session. Each request is HTTP/1.1 on a connection of its own, which the client
//! closes.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// How long ChromeDriver may take to report itself ready.
const DRIVER_START: Duration = Duration::from_secs(30);

/// A free port of localhost, as the system hands one out; it is free until another process
/// binds it, a race the test accepts.
pub fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .expect("the system hands out a free port of localhost")
}

/// A child process killed when the test is done with it, however it ends.
pub struct Process(pub Child);

impl Drop for Process {
    fn drop(&mut self) {
        // It may have ended already; there is nothing left to do then.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The ChromeDriver binary: in the directory `CHROMEWEBDRIVER` names, as GitHub's runner image
/// sets it, else the first `chromedriver` on `PATH`.
pub fn chromedriver() -> Option<PathBuf> {
    let named = std::env::var_os("CHROMEWEBDRIVER")
        .map(|dir| PathBuf::from(dir).join("chromedriver"))
        .filter(|path| path.is_file());
    named.or_else(|| {
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|dir| dir.join("chromedriver"))
            .find(|path| path.is_file())
    })
}

/// A running ChromeDriver and the address it listens on.
pub struct Driver {
    address: SocketAddr,
    _process: Process,
}

impl Driver {
    /// Starts `binary` on a free port and waits until its status reports it ready.
    pub fn start(binary: &PathBuf) -> Result<Self, String> {
        let port = free_port();
        let child = Command::new(binary)
            .arg(format!("--port={port}"))
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| format!("{} does not start: {e}", binary.display()))?;
        let driver = Self {
            address: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            _process: Process(child),
        };
        let deadline = Instant::now() + DRIVER_START;
        loop {
            match request(driver.address, "GET", "/status", None) {
                Ok(status) if status["ready"] == json!(true) => return Ok(driver),
                _ if Instant::now() > deadline => {
                    return Err(format!("ChromeDriver is not ready after {DRIVER_START:?}"));
                }
                _ => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    }

    /// New Session with headless Chrome. A missing Chrome fails here, in the driver's words.
    pub fn session(&self) -> Result<Session, String> {
        let capabilities = json!({"capabilities": {"alwaysMatch": {
            "browserName": "chrome",
            "goog:chromeOptions": {"args": ["--headless=new"]},
        }}});
        let value = request(self.address, "POST", "/session", Some(&capabilities))?;
        let id = value["sessionId"]
            .as_str()
            .ok_or_else(|| format!("New Session returned no session id: {value}"))?;
        Ok(Session {
            address: self.address,
            id: id.to_owned(),
        })
    }
}

/// A session of the browser, deleted when dropped.
pub struct Session {
    address: SocketAddr,
    id: String,
}

impl Session {
    /// Navigate To `url`; returns once the page has loaded.
    pub fn navigate(&self, url: &str) -> Result<(), String> {
        let path = format!("/session/{}/url", self.id);
        request(self.address, "POST", &path, Some(&json!({"url": url}))).map(drop)
    }

    /// Execute Script: runs `script`, a function body, with `args`, and returns what it returns.
    pub fn execute(&self, script: &str, args: &[Value]) -> Result<Value, String> {
        let path = format!("/session/{}/execute/sync", self.id);
        let body = json!({"script": script, "args": args});
        request(self.address, "POST", &path, Some(&body))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Delete Session; the driver ends the browser with it. A failure leaves only the driver's
        // process, which its guard kills.
        let _ = request(
            self.address,
            "DELETE",
            &format!("/session/{}", self.id),
            None,
        );
    }
}

/// Sends one request to the driver and returns the `value` of its answer, or the protocol's
/// error with its message.
pub fn request(
    address: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Result<Value, String> {
    let body = body.map(Value::to_string).unwrap_or_default();
    let mut stream = TcpStream::connect(address).map_err(|e| format!("{method} {path}: {e}"))?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json; \
         charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .map_err(|e| format!("{method} {path}: {e}"))?;
    let (status, answer) = read_response(&stream).map_err(|e| format!("{method} {path}: {e}"))?;
    let answer: Value = serde_json::from_slice(&answer)
        .map_err(|e| format!("{method} {path}: HTTP {status}, no JSON: {e}"))?;
    let value = answer["value"].clone();
    if status == 200 {
        Ok(value)
    } else {
        Err(format!(
            "{method} {path}: HTTP {status}, {}: {}",
            value["error"], value["message"]
        ))
    }
}

/// Reads a response to its end: its status code and its body, of its `Content-Length` or up to
/// the close of the connection. ChromeDriver answers with a `Content-Length`.
fn read_response(stream: &TcpStream) -> std::io::Result<(u16, Vec<u8>)> {
    let invalid =
        |what: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_owned());
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| invalid("no status line"))?;
    let mut length = None;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim_end().is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').unwrap_or((&line, ""));
        let (name, value) = (name.trim().to_ascii_lowercase(), value.trim());
        if name == "content-length" {
            length = Some(value.parse().map_err(|_| invalid("bad Content-Length"))?);
        }
    }
    let mut body = Vec::new();
    if let Some(length) = length {
        body.resize(length, 0);
        reader.read_exact(&mut body)?;
    } else {
        reader.read_to_end(&mut body)?;
    }
    Ok((status, body))
}
