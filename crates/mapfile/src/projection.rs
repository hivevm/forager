// The reading of a PROJECTION block follows MapServer's loadProjection, src/mapfile.c, of the
// release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

//! The parameters of a `PROJECTION` block, read once: for the init files the frontend reads before
//! translation, and for the proj4 string the translation builds from them (ADR-0019).

use forager_style_ir::Span;

use crate::syntax::Block;

/// What a `PROJECTION` block's parameters are.
pub enum Projection {
    /// A first token that is `AUTO` and nothing else, in any case: the projection the source
    /// declares, as `msProcessProjection` leaves it to the source.
    Auto,
    /// The explicit parameters, each with a leading `+`, and at most one `init=` reference.
    Parameters {
        explicit: Vec<String>,
        init: Option<Init>,
    },
}

/// An `init=` reference.
pub struct Init {
    /// The parameter as written, without its `+`, for the diagnostics that name it.
    pub param: String,
    /// The file or authority and the code, split at the last `:` as PROJ splits it
    /// (`get_init_string`); `None` where the reference has no `:`.
    pub reference: Option<(String, String)>,
    pub span: Span,
}

impl Init {
    /// The init file the reference names by a path: its file name, where that holds a `/`.
    pub fn file(&self) -> Option<&str> {
        let (file, _) = self.reference.as_ref()?;
        file.contains('/').then_some(file.as_str())
    }
}

/// A parameter that is no projection's, with its span.
pub struct Invalid {
    pub message: &'static str,
    pub span: Span,
}

/// Reads a `PROJECTION` block's parameters. As MapServer hands every parameter to PROJ, each is
/// written with a leading `+`, `init=` is matched case-sensitively, and there is at most one.
pub fn read(block: &Block) -> Result<Projection, Invalid> {
    let mut explicit = Vec::new();
    let mut init: Option<Init> = None;
    for token in &block.values {
        // loadProjection, src/mapfile.c, takes a string or the keyword `auto`, MS_AUTO.
        let auto = token.text().filter(|w| w.eq_ignore_ascii_case("auto"));
        let Some(text) = token.string().or(auto).map(str::trim) else {
            return Err(Invalid {
                message: "a projection parameter is a string",
                span: token.span.clone(),
            });
        };
        if explicit.is_empty() && init.is_none() && text.eq_ignore_ascii_case("AUTO") {
            return Ok(Projection::Auto);
        }
        for param in text.split_whitespace() {
            let param = param.trim_start_matches('+');
            let Some(reference) = param.strip_prefix("init=") else {
                explicit.push(format!("+{param}"));
                continue;
            };
            if init.is_some() {
                return Err(Invalid {
                    message: "a PROJECTION has at most one init=",
                    span: token.span.clone(),
                });
            }
            init = Some(Init {
                param: param.to_owned(),
                reference: reference
                    .rsplit_once(':')
                    .map(|(file, code)| (file.to_owned(), code.to_owned())),
                span: token.span.clone(),
            });
        }
    }
    Ok(Projection::Parameters { explicit, init })
}
