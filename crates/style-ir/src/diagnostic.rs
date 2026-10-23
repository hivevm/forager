use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Where an IR element or a diagnostic comes from: a file, a line, and a column, both 1-based.
///
/// A span serializes as `file:line:column`, so the style document stays readable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl Span {
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

impl Serialize for Span {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Span {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        // The file name may itself contain colons, so the two numbers are taken from the right.
        let mut parts = text.rsplitn(3, ':');
        let column = parts.next().and_then(|c| c.parse().ok());
        let line = parts.next().and_then(|l| l.parse().ok());
        let file = parts.next();
        match (file, line, column) {
            (Some(file), Some(line), Some(column)) => Ok(Span::new(file, line, column)),
            _ => Err(serde::de::Error::custom(format!(
                "a span is `file:line:column`, found `{text}`"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        })
    }
}

/// A message from loading or rendering. Errors from user data are reported as diagnostics, never
/// as panics (Q-3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// A stable, kebab-case identifier a test or a tool can match on.
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn error(code: &str, message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.to_owned(),
            message: message.into(),
            span,
        }
    }

    pub fn warning(code: &str, message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.to_owned(),
            message: message.into(),
            span,
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(span) = &self.span {
            write!(f, "{span}: ")?;
        }
        write!(f, "{} [{}]: {}", self.severity, self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_round_trips_through_its_text_form_even_with_colons_in_the_file() {
        let span = Span::new("C:/maps/world.map", 12, 3);
        let json = serde_json::to_string(&span).unwrap();
        assert_eq!(json, "\"C:/maps/world.map:12:3\"");
        assert_eq!(serde_json::from_str::<Span>(&json).unwrap(), span);
    }

    #[test]
    fn a_span_without_numbers_is_rejected() {
        assert!(serde_json::from_str::<Span>("\"world.map\"").is_err());
    }
}
