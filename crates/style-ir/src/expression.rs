//! Expressions of the style IR: a lean CQL2 tree, serialized in the shape of CQL2-JSON
//! (ADR-0019). What CQL2 lacks is written as functions in the same `{"op", "args"}` shape.

use serde::{Deserialize, Serialize};

/// How deep an expression may nest (Q-9). A deeper one is refused where it is read, from a
/// mapfile or from a style document. Each level is two levels of a YAML or JSON document, so
/// a document holding the deepest expression stays inside the nesting limits of both readers.
pub const MAX_EXPRESSION_DEPTH: usize = 24;

/// An expression: a literal, a property of the feature, a list, a time, or an operator or
/// function applied to its arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
