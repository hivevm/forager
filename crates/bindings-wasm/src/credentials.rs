//! The credential filter of the browser (Q-7). The browser binds its data by layer name and
//! never uses a source reference, so it admits only the one property that names a file, `data`,
//! and only when that carries nothing that looks like a secret. Everything else — CONNECTION
//! above all, whatever its form — is refused, so no heuristic has to recognize every way to
//! write a password.

use forager_style_ir::SourceRef;

/// Parameter names whose value is a secret, as they appear in URLs and connection strings.
const SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "pass",
    "secret",
    "token",
    "apikey",
    "api_key",
    "access_key",
    "key",
    "sig",
    "signature",
];

/// Whether the browser may keep `source`: only a `data` property, without a secret in it.
pub fn admissible(source: &SourceRef) -> bool {
    source
        .props
        .iter()
        .all(|(key, value)| key == "data" && !has_secret_parameter(value) && !has_userinfo(value))
}

/// `name = value` anywhere, `name` ending in one of the secret keys, spaces around `=` allowed.
fn has_secret_parameter(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.match_indices('=').any(|(eq, _)| {
        let before = lower[..eq].trim_end();
        // Past the whole character that ends the name, which may take more than one byte.
        let name_start = before
            .char_indices()
            .rfind(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let name = &before[name_start..];
        !name.is_empty() && SECRET_KEYS.iter().any(|secret| name.ends_with(secret))
    })
}

/// User information before an `@`: `scheme://user:pass@host`, or Oracle's `user/pass@db`.
fn has_userinfo(text: &str) -> bool {
    text.split_whitespace().any(|word| {
        word.rsplit_once('@').is_some_and(|(before, _)| {
            let userinfo = before.rsplit("://").next().unwrap_or(before);
            userinfo.contains(':') || userinfo.contains('/')
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(props: &[(&str, &str)]) -> SourceRef {
        SourceRef {
            kind: "local".into(),
            props: props
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            span: None,
        }
    }

    #[test]
    fn a_connection_is_refused_whatever_it_holds() {
        // Verifies: Q-7
        for connection in [
            "host=db dbname=gis user=u password = secret",
            "host=db dbname=gis user=reader",
            "scott/tiger@orcl",
            "https://tiles.example.org/{z}/{x}/{y}.pbf",
        ] {
            assert!(
                !admissible(&source(&[("connection", connection)])),
                "{connection}"
            );
        }
        assert!(!admissible(&source(&[
            ("data", "a.shp"),
            ("db_password", "x")
        ])));
    }

    #[test]
    fn data_with_a_secret_is_refused() {
        // Verifies: Q-7
        for data in [
            "PG:host=db password\t=secret",
            "PG:host=db PWD=x",
            "https://u:pa/ss@host/a.fgb",
            "https://host/a.fgb?key=1",
            "https://host/a.fgb?sv=1&sig=abc",
            "https://host/a.fgb?X-Amz-Signature=abc",
            "scott/tiger@orcl",
        ] {
            assert!(!admissible(&source(&[("data", data)])), "{data}");
        }
    }

    #[test]
    fn plain_data_is_kept() {
        for data in [
            "countries.shp",
            "areas.geojson",
            "the_geom from (select * from passwords_table) as t",
            "https://host/tiles/a.fgb",
        ] {
            assert!(admissible(&source(&[("data", data)])), "{data}");
        }
        assert!(admissible(&source(&[])));
    }

    /// A character of more than one byte before `=` is stepped past whole, and decides as an ASCII
    /// one in its place does.
    #[test]
    fn data_with_any_character_before_an_equals_sign_is_read_without_a_panic() {
        // Verifies: Q-3, Q-7
        assert!(admissible(&source(&[("data", "ü=1.shp")])));
        assert!(admissible(&source(&[("data", "data/ü=1.shp")])));
        assert!(admissible(&source(&[("data", "u=1.shp")])));
        assert!(!admissible(&source(&[("data", "€password=x")])));
        assert!(!admissible(&source(&[("data", "#password=x")])));
    }
}
