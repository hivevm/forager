//! The parameters of a `PROJECTION` block, read once: for the init files the frontend reads before
//! translation, and for the proj4 string the translation builds from them (ADR-0023).

use forager_style_ir::Span;

use crate::lexer::TokenKind;
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
        let text = match &token.kind {
            TokenKind::Str { value, .. } | TokenKind::Word(value) => value.trim(),
            _ => {
                return Err(Invalid {
                    message: "a projection parameter is a string",
                    span: token.span.clone(),
                });
            }
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
