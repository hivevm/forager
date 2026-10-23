//! The credential filter of the browser (Q-7): a source reference that carries a credential is
//! never used in the client. The host should have rewritten it on the server; if it did not,
//! the bindings drop it and say so.

use forager_style_ir::SourceRef;

/// Parameter names whose value is a secret, as they appear in connection strings
/// (`password=…`) and URLs (`?apikey=…`).
const SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "secret",
    "token",
    "apikey",
    "api_key",
    "access_key",
];

/// Whether any property of `source` carries a credential: a secret parameter, a property named
/// like one, or a URL with a password in its user information (`scheme://user:pass@host`).
pub fn carries_credentials(source: &SourceRef) -> bool {
    source.props.iter().any(|(key, value)| {
        is_secret_key(key) || has_secret_parameter(value) || has_url_password(value)
    })
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    SECRET_KEYS.iter().any(|secret| key.ends_with(secret))
}

/// `name=value` anywhere in the text, `name` one of the secret keys, delimited on its left by
/// the start, whitespace, `;`, `&`, `?`, or `:`.
fn has_secret_parameter(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.match_indices('=').any(|(eq, _)| {
        let start = lower[..eq]
            .rfind(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '?' | ':'))
            .map_or(0, |i| i + 1);
        is_secret_key(lower[start..eq].trim())
    })
}

fn has_url_password(text: &str) -> bool {
    text.match_indices("://").any(|(i, _)| {
        let rest = &text[i + 3..];
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        authority
            .rsplit_once('@')
            .is_some_and(|(userinfo, _)| userinfo.contains(':'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(key: &str, value: &str) -> SourceRef {
        SourceRef {
            kind: "postgis".into(),
            props: [(key.to_owned(), value.to_owned())].into(),
            span: None,
        }
    }

    #[test]
    fn credentials_in_connection_strings_and_urls_are_found() {
        // Verifies: Q-7
        for (key, value) in [
            ("connection", "host=db dbname=gis user=u password=secret"),
            ("connection", "PG:host=db PWD=x"),
            ("connection", "Server=db;Uid=u;Pwd=x"),
            ("connection", "https://u:p@tiles.example.org/{z}/{x}/{y}"),
            (
                "connection",
                "https://tiles.example.org/wms?layers=a&apikey=123",
            ),
            ("connection", "https://tiles.example.org/wms?ACCESS_KEY=1"),
            ("db_password", "x"),
        ] {
            assert!(carries_credentials(&source(key, value)), "{key}={value}");
        }
    }

    #[test]
    fn plain_sources_pass() {
        for (key, value) in [
            ("data", "countries.shp"),
            ("connection", "host=db dbname=gis user=reader"),
            ("connection", "https://tiles.example.org/{z}/{x}/{y}.pbf"),
            ("connection", "https://user@tiles.example.org/a"),
            ("data", "the_geom from (select * from passwords_table) as t"),
        ] {
            assert!(!carries_credentials(&source(key, value)), "{key}={value}");
        }
    }
}
