//! Expressions of the style IR: a lean CQL2 tree, serialized in the shape of CQL2-JSON
//! (ADR-0019). What CQL2 lacks is written as functions in the same `{"op", "args"}` shape.

use std::fmt;

use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// How deep an expression may nest (Q-9). A deeper one is refused where it is read, from a
/// mapfile or from a style document. Each level is two levels of a YAML or JSON document, so
/// a document holding the deepest expression stays inside the nesting limits of both readers.
pub const MAX_EXPRESSION_DEPTH: usize = 24;

/// An expression: a literal, a property of the feature, a list, a time, or an operator or
/// function applied to its arguments. It is read by the keys of its object, as CQL2-JSON tells
/// its alternatives apart, so that a key it does not know is named like any other field.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum Expr {
    Bool(bool),
    Number(f64),
    String(String),
    /// The second argument of `in`.
    List(Vec<Expr>),
    Call {
        op: Op,
        args: Vec<Expr>,
    },
    Property {
        property: String,
    },
    Date {
        date: String,
    },
    Timestamp {
        timestamp: String,
    },
}

/// The operators of CQL2 an expression uses, and the functions for what CQL2 lacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Op {
    #[serde(rename = "=")]
    Eq,
    #[serde(rename = "<>")]
    Ne,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "in")]
    In,
    #[serde(rename = "and")]
    And,
    #[serde(rename = "or")]
    Or,
    #[serde(rename = "not")]
    Not,
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "-")]
    Sub,
    #[serde(rename = "*")]
    Mul,
    #[serde(rename = "/")]
    Div,
    #[serde(rename = "%")]
    Mod,
    #[serde(rename = "^")]
    Pow,
    #[serde(rename = "casei")]
    CaseI,
    #[serde(rename = "t_before")]
    TBefore,
    #[serde(rename = "t_after")]
    TAfter,
    #[serde(rename = "t_equals")]
    TEquals,
    /// A property's value as a number, as C's `atof` reads it: its longest numeric prefix, or 0.
    #[serde(rename = "to_number")]
    ToNumber,
    #[serde(rename = "to_string")]
    ToString,
    /// A property's value as a time, as MapServer's `msParseTime` reads it.
    #[serde(rename = "to_timestamp")]
    ToTimestamp,
    /// `regexp_like(value, pattern, flags)`: the pattern in POSIX extended syntax, the flag `i`.
    #[serde(rename = "regexp_like")]
    RegexpLike,
    #[serde(rename = "concat")]
    Concat,
    #[serde(rename = "upper")]
    Upper,
    #[serde(rename = "lower")]
    Lower,
    #[serde(rename = "initcap")]
    Initcap,
    #[serde(rename = "firstcap")]
    Firstcap,
    #[serde(rename = "commify")]
    Commify,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "round")]
    Round,
    /// `tostring(number, format)`: the number formatted by a C `printf` format.
    #[serde(rename = "tostring")]
    Tostring,
}

impl Expr {
    /// An operator or function applied to `args`.
    pub fn call(op: Op, args: Vec<Expr>) -> Self {
        Expr::Call { op, args }
    }

    pub fn property(name: impl Into<String>) -> Self {
        Expr::Property {
            property: name.into(),
        }
    }

    /// How deep the tree nests: a literal or a property is 1. Iterative, so that measuring a
    /// tree too deep to accept cannot overflow the stack.
    pub fn depth(&self) -> usize {
        let mut deepest = 0;
        let mut pending = vec![(self, 1)];
        while let Some((expr, depth)) = pending.pop() {
            deepest = deepest.max(depth);
            match expr {
                Expr::Call { args, .. } | Expr::List(args) => {
                    pending.extend(args.iter().map(|arg| (arg, depth + 1)));
                }
                _ => {}
            }
        }
        deepest
    }
}

/// The keys of an expression's object, or of a binding's, read one by one. A key this build
/// does not know is read as ignored, so that the style document's reader names it with its path
/// (ADR-0020), where an untagged enum would have skipped it unseen.
#[derive(Default)]
pub(crate) struct Keys {
    op: Option<Op>,
    args: Option<Vec<Expr>>,
    property: Option<String>,
    date: Option<String>,
    timestamp: Option<String>,
    pub(crate) binding: Option<String>,
}

/// The error of an object that is no expression, worded as serde words that of an untagged
/// enum, which the reader of a newer minor recognizes.
pub(crate) fn no_variant<E: de::Error>(name: &str) -> E {
    E::custom(format!(
        "data did not match any variant of untagged enum {name}"
    ))
}

impl Keys {
    /// Reads the keys of `map`; `binding` is a key only where a binding may stand.
    pub(crate) fn read<'de, A: MapAccess<'de>>(
        mut map: A,
        binding: bool,
    ) -> Result<Self, A::Error> {
        /// A key's value, refused the second time, as a derived struct refuses a field twice.
        fn once<'de, A: MapAccess<'de>, T: Deserialize<'de>>(
            map: &mut A,
            slot: &mut Option<T>,
            key: &'static str,
        ) -> Result<(), A::Error> {
            if slot.is_some() {
                return Err(de::Error::duplicate_field(key));
            }
            *slot = Some(map.next_value()?);
            Ok(())
        }
        let mut keys = Keys::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "op" => once(&mut map, &mut keys.op, "op")?,
                "args" => once(&mut map, &mut keys.args, "args")?,
                "property" => once(&mut map, &mut keys.property, "property")?,
                "date" => once(&mut map, &mut keys.date, "date")?,
                "timestamp" => once(&mut map, &mut keys.timestamp, "timestamp")?,
                "binding" if binding => once(&mut map, &mut keys.binding, "binding")?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        // The keys of one alternative only: an object with those of two is none of them.
        let kinds = [
            keys.op.is_some() || keys.args.is_some(),
            keys.property.is_some(),
            keys.date.is_some(),
            keys.timestamp.is_some(),
            keys.binding.is_some(),
        ];
        if kinds.iter().filter(|kind| **kind).count() > 1 {
            let name = if binding { "Bound" } else { "Expr" };
            let error: A::Error = no_variant(name);
            return Err(de::Error::custom(format!(
                "{error}: the object holds the keys of more than one alternative"
            )));
        }
        Ok(keys)
    }

    /// The expression the keys make, or the error `name` gives for an object that is none.
    pub(crate) fn expr<E: de::Error>(self, name: &str) -> Result<Expr, E> {
        match self {
            Keys {
                op: Some(op),
                args: Some(args),
                ..
            } => Ok(Expr::Call { op, args }),
            Keys {
                property: Some(property),
                ..
            } => Ok(Expr::Property { property }),
            Keys {
                date: Some(date), ..
            } => Ok(Expr::Date { date }),
            Keys {
                timestamp: Some(timestamp),
                ..
            } => Ok(Expr::Timestamp { timestamp }),
            _ => Err(no_variant(name)),
        }
    }
}

struct ExprVisitor;

impl<'de> Visitor<'de> for ExprVisitor {
    type Value = Expr;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an expression")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Expr, E> {
        Ok(Expr::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Expr, E> {
        Ok(Expr::Number(v as f64))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Expr, E> {
        Ok(Expr::Number(v as f64))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Expr, E> {
        Ok(Expr::Number(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Expr, E> {
        Ok(Expr::String(v.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Expr, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Expr::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Expr, A::Error> {
        Keys::read(map, false)?.expr("Expr")
    }
}

impl<'de> Deserialize<'de> for Expr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ExprVisitor)
    }
}

impl From<&str> for Expr {
    fn from(text: &str) -> Self {
        Expr::String(text.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_serialize_in_the_shape_of_cql2_json() {
        // Verifies: ADR-0019
        let expr = Expr::call(
            Op::And,
            vec![
                Expr::call(
                    Op::Eq,
                    vec![
                        Expr::call(Op::CaseI, vec![Expr::property("type")]),
                        Expr::call(Op::CaseI, vec!["road".into()]),
                    ],
                ),
                Expr::call(
                    Op::RegexpLike,
                    vec![
                        Expr::call(Op::ToString, vec![Expr::property("name")]),
                        "^r.*d$".into(),
                        "i".into(),
                    ],
                ),
                Expr::call(
                    Op::In,
                    vec![
                        Expr::property("a"),
                        Expr::List(vec!["A".into(), "B".into()]),
                    ],
                ),
                Expr::call(
                    Op::TBefore,
                    vec![
                        Expr::Timestamp {
                            timestamp: "2010-12-01T10:00:00Z".into(),
                        },
                        Expr::Date {
                            date: "2010-12-02".into(),
                        },
                    ],
                ),
                Expr::Bool(true),
                Expr::Number(1.5),
            ],
        );
        let json = serde_json::to_string(&expr).unwrap();
        assert_eq!(
            json,
            r#"{"op":"and","args":[{"op":"=","args":[{"op":"casei","args":[{"property":"type"}]},{"op":"casei","args":["road"]}]},{"op":"regexp_like","args":[{"op":"to_string","args":[{"property":"name"}]},"^r.*d$","i"]},{"op":"in","args":[{"property":"a"},["A","B"]]},{"op":"t_before","args":[{"timestamp":"2010-12-01T10:00:00Z"},{"date":"2010-12-02"}]},true,1.5]}"#
        );
        assert_eq!(serde_json::from_str::<Expr>(&json).unwrap(), expr);
    }

    #[test]
    fn depth_counts_the_levels_of_the_tree() {
        assert_eq!(Expr::Bool(true).depth(), 1);
        let not_not = Expr::call(Op::Not, vec![Expr::call(Op::Not, vec![Expr::Bool(true)])]);
        assert_eq!(not_not.depth(), 3);
        let list = Expr::call(Op::In, vec!["a".into(), Expr::List(vec!["a".into()])]);
        assert_eq!(list.depth(), 3);
    }
}
