// Which output format a map selects, and whether it is transparent, follows src/mapoutput.c and
// src/mapfile.c of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 2002, Frank Warmerdam (src/mapoutput.c).
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! Whether the image a map is drawn on starts transparent: the `TRANSPARENT` of the output format
//! its `IMAGETYPE` selects, as `msPostMapParseOutputFormatSetup`, `msSelectOutputFormat`, and
//! `msGetOutputFormatIndex` of src/mapoutput.c select it, and `loadOutputFormat` of src/mapfile.c
//! and `msOutputFormatValidate` set it. `agg2CreateImage`, src/mapagg.cpp, clears the image of a
//! transparent format to no colour, whatever the map's `IMAGECOLOR`.

use crate::syntax::{Block, Directive, Node};

/// What an `OUTPUTFORMAT` block contributes to the selection: the names it is found by, and
/// whether it is transparent.
struct Format {
    name: Option<String>,
    mime_type: Option<String>,
    transparent: bool,
}

/// The single string or word of a directive, as `getString` reads it.
fn value(d: &Directive) -> Option<String> {
    match d.values.as_slice() {
        [token] => token.string().map(str::to_owned),
        _ => None,
    }
}

/// The name and the MIME type `msCreateDefaultOutputFormat` gives a format of `driver`, for the
/// drivers whose formats are PNG or JPEG images; a `GD/` driver falls back on `AGG/PNG8`.
fn driver_defaults(driver: &str) -> (Option<&'static str>, Option<&'static str>) {
    let driver = driver.to_ascii_uppercase();
    match driver.as_str() {
        "AGG/PNG" => (Some("png"), Some("image/png")),
        "AGG/JPEG" => (Some("jpeg"), Some("image/jpeg")),
        "AGG/PNG8" => (Some("png8"), Some("image/png; mode=8bit")),
        _ if driver.starts_with("GD/") => (Some("png8"), Some("image/png; mode=8bit")),
        _ => (None, None),
    }
}

/// An `OUTPUTFORMAT` block as `loadOutputFormat` reads it: the last of each keyword holds. Without
/// `TRANSPARENT`, an `IMAGEMODE` of `RGBA` makes it transparent and any other not; a format whose
/// driver writes JPEG is never transparent (`msOutputFormatValidate`).
fn format(block: &Block) -> Format {
    let mut driver = None;
    let mut name = None;
    let mut mime_type = None;
    let mut image_mode = None;
    let mut transparent = None;
    for node in &block.nodes {
        let Node::Directive(d) = node else { continue };
        match d.keyword.as_str() {
            "DRIVER" => driver = value(d),
            "NAME" => name = value(d),
            "MIMETYPE" => mime_type = value(d),
            "IMAGEMODE" => image_mode = value(d).map(|m| m.to_ascii_uppercase()),
            "TRANSPARENT" => {
                transparent = match value(d).map(|v| v.to_ascii_uppercase()).as_deref() {
                    Some("ON" | "TRUE") => Some(true),
                    Some("OFF" | "FALSE") => Some(false),
                    _ => transparent,
                };
            }
            _ => {}
        }
    }
    let driver = driver.unwrap_or_default();
    let (default_name, default_mime_type) = driver_defaults(&driver);
    let jpeg = driver
        .rsplit_once('/')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("JPEG"));
    let transparent = transparent.unwrap_or(image_mode.as_deref() == Some("RGBA")) && !jpeg;
    Format {
        name: name.or_else(|| default_name.map(str::to_owned)),
        mime_type: mime_type.or_else(|| default_mime_type.map(str::to_owned)),
        transparent,
    }
}

/// Whether the output format the `MAP` block selects is transparent. Its `IMAGETYPE`, or without
/// one the name of its first `OUTPUTFORMAT`, finds a format of the map first by its MIME type,
/// then by its name, each without regard to case; one it does not find is a built-in format, and
/// none of those is transparent.
pub(super) fn transparent(map: &Block) -> bool {
    let formats: Vec<Format> = map
        .nodes
        .iter()
        .filter_map(|node| match node {
            Node::Block(b) if b.keyword == "OUTPUTFORMAT" => Some(format(b)),
            _ => None,
        })
        .collect();
    let image_type = map.nodes.iter().rev().find_map(|node| match node {
        Node::Directive(d) if d.keyword == "IMAGETYPE" => value(d),
        _ => None,
    });
    let Some(image_type) = image_type.or_else(|| formats.first().and_then(|f| f.name.clone()))
    else {
        return false;
    };
    let by = |field: fn(&Format) -> Option<&String>| {
        formats
            .iter()
            .find(|f| field(f).is_some_and(|value| value.eq_ignore_ascii_case(&image_type)))
    };
    by(|f| f.mime_type.as_ref())
        .or_else(|| by(|f| f.name.as_ref()))
        .is_some_and(|f| f.transparent)
}
