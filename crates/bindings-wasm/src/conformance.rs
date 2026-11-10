//! The module the conformance test runs in an interpreter and compares with the native render
//! (G-9, ADR-0034): a style document with its features inline (ADR-0018) and the files its render
//! reads, rendered by the engine and the reference renderer, built only with the feature
//! `conformance`. The workspace denies `unsafe` code, so the exports take and give plain integers
//! through `wasm_bindgen`, and the host writes the input into, and reads the output from, the
//! module's memory at the addresses they return:
//!
//! 1. `conformance_input(len)` makes room for an input of `len` bytes and returns its address;
//! 2. `conformance_render()` renders the input and returns the length of the output;
//! 3. `conformance_output()` returns the output's address.
//!
//! The input is the document's JSON, then the files, each integer a little-endian `u32`: the
//! JSON's length and bytes; the count of files; per file its path's length and bytes, `1` and the
//! file's length and bytes for a file the loader gives, or `0` and the length and bytes of the
//! error it gives instead. The output is the image's width and height, each a little-endian `u32`,
//! and its RGBA bytes; a width of 0 is no image, followed by why, as UTF-8 text.

use std::collections::BTreeMap;

use forager_adapter_geojson::GeoJsonSource;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render_agg::AggRenderer;
use forager_style_ir::StyleDocument;

/// The files of a render by their paths: each the bytes a loader gave, or the error it gave.
pub type Files = BTreeMap<String, Result<Vec<u8>, String>>;

/// The document and the files of `input`, or why it is not an input.
pub fn decode_input(input: &[u8]) -> Result<(String, Files), String> {
    let mut at = 0;
    let mut take = |n: usize| -> Result<&[u8], String> {
        let end = at + n;
        let taken = input.get(at..end).ok_or("the input ends early")?;
        at = end;
        Ok(taken)
    };
    fn number(b: &[u8]) -> usize {
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize
    }
    let text = |b: &[u8]| String::from_utf8(b.to_vec()).map_err(|e| e.to_string());
    let n = number(take(4)?);
    let document = text(take(n)?)?;
    let count = number(take(4)?);
    let mut files = Files::new();
    for _ in 0..count {
        let n = number(take(4)?);
        let path = text(take(n)?)?;
        let ok = take(1)?[0] == 1;
        let n = number(take(4)?);
        let bytes = take(n)?.to_vec();
        files.insert(path, if ok { Ok(bytes) } else { Err(text(&bytes)?) });
    }
    Ok((document, files))
}

/// The loader of the files the input holds; any other path is the error a missing file gives.
struct Given<'a>(&'a Files);

impl ResourceLoader for Given<'_> {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        match self.0.get(path.as_str()) {
            Some(Ok(bytes)) => Ok(bytes.clone()),
            Some(Err(message)) => Err(SourceError::new(message.clone())),
            None => Err(SourceError::new("the input holds no file of this path")),
        }
    }
}

/// Renders `document`, read from JSON, with `files`, as the module and the native test both do:
/// with no source bound, every layer read from its inline features. The output is in the module's
/// format.
pub fn render_document(document: &str, files: &Files) -> Vec<u8> {
    let read = match StyleDocument::from_json(document) {
        Ok(read) => read,
        Err(e) => return no_image(&format!("the document: {e}")),
    };
    let sources: BTreeMap<String, GeoJsonSource> = BTreeMap::new();
    let Some(rendered) = forager_ports::poll_once(forager_engine::render_with_files(
        &read.document.map,
        &sources,
        &Given(files),
        AggRenderer::new(),
    )) else {
        return no_image("a port waited");
    };
    let Some(image) = rendered.image else {
        let errors: Vec<_> = rendered
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect();
        return no_image(&errors.join("; "));
    };
    let mut output = Vec::with_capacity(8 + image.pixels.len());
    output.extend(image.width.to_le_bytes());
    output.extend(image.height.to_le_bytes());
    output.extend(image.pixels);
    output
}

fn no_image(why: &str) -> Vec<u8> {
    let mut output = vec![0; 8];
    output.extend(why.as_bytes());
    output
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// The input the host writes, then the output it reads.
    static BUFFER: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Makes room for an input of `len` bytes and returns its address in the module's memory.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn conformance_input(len: u32) -> u32 {
    BUFFER.with_borrow_mut(|buffer| {
        *buffer = vec![0; len as usize];
        address(buffer)
    })
}

/// Renders the input written at [`conformance_input`]'s address and returns the output's length.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn conformance_render() -> u32 {
    BUFFER.with_borrow_mut(|buffer| {
        *buffer = match decode_input(buffer) {
            Ok((document, files)) => render_document(&document, &files),
            Err(why) => no_image(&format!("the input: {why}")),
        };
        u32::try_from(buffer.len()).unwrap_or(u32::MAX)
    })
}

/// The address of the output [`conformance_render`] left.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn conformance_output() -> u32 {
    BUFFER.with_borrow_mut(|buffer| address(buffer))
}

/// The address of `buffer`'s bytes in the 32-bit memory of the module.
#[cfg(target_arch = "wasm32")]
fn address(buffer: &mut [u8]) -> u32 {
    buffer.as_mut_ptr() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The input `files` and `document` are handed to the module as.
    fn encode_input(document: &str, files: &Files) -> Vec<u8> {
        let mut input = Vec::new();
        let mut bytes = |b: &[u8]| {
            input.extend(len(b.len()).to_le_bytes());
            input.extend(b);
        };
        bytes(document.as_bytes());
        let mut rest = Vec::new();
        rest.extend(len(files.len()).to_le_bytes());
        for (path, file) in files {
            rest.extend(len(path.len()).to_le_bytes());
            rest.extend(path.as_bytes());
            let (ok, b) = match file {
                Ok(b) => (1u8, b.as_slice()),
                Err(e) => (0u8, e.as_bytes()),
            };
            rest.push(ok);
            rest.extend(len(b.len()).to_le_bytes());
            rest.extend(b);
        }
        input.extend(rest);
        input
    }

    fn len(n: usize) -> u32 {
        u32::try_from(n).unwrap()
    }

    #[test]
    fn an_input_reads_back_as_its_document_and_files() {
        let mut files = Files::new();
        files.insert("fonts/a.ttf".to_owned(), Ok(vec![1, 2, 3]));
        files.insert("missing.png".to_owned(), Err("not found".to_owned()));
        let input = encode_input("{\"x\": 1}", &files);
        assert_eq!(
            decode_input(&input),
            Ok(("{\"x\": 1}".to_owned(), files.clone()))
        );
        assert_eq!(
            decode_input(&input[..input.len() - 1]),
            Err("the input ends early".to_owned())
        );
    }
}
