//! Expression trees and feature properties built from fuzzed bytes, for the targets that evaluate
//! the style IR's expressions. A tree is read as a list of nodes in postfix order, so that reading
//! it never recurses, and is cut at the depth the IR allows.

use arbitrary::Arbitrary;
use forager_ports::{Properties, Stored};
use forager_style_ir::{Expr, MAX_EXPRESSION_DEPTH, Op};

/// Every operator and function of the IR.
const OPS: &[Op] = &[
    Op::Eq,
    Op::Ne,
    Op::Lt,
    Op::Gt,
    Op::Le,
    Op::Ge,
    Op::In,
    Op::And,
    Op::Or,
    Op::Not,
    Op::Add,
    Op::Sub,
    Op::Mul,
    Op::Div,
    Op::Mod,
    Op::Pow,
    Op::CaseI,
    Op::TBefore,
    Op::TAfter,
    Op::TEquals,
    Op::ToNumber,
    Op::ToString,
    Op::ToTimestamp,
    Op::RegexpLike,
    Op::Concat,
    Op::Upper,
    Op::Lower,
    Op::Initcap,
    Op::Firstcap,
    Op::Commify,
    Op::Length,
    Op::Round,
    Op::Tostring,
];

/// The names of the properties a tree reads and a feature holds.
pub const NAMES: &[&str] = &["a", "B", "c"];

#[derive(Arbitrary, Debug)]
pub enum Node {
    Bool(bool),
    Number(f64),
    String(String),
    Property(u8),
    Date(String),
    Timestamp(String),
    /// The last `n` trees as a list.
    List(u8),
    /// An operator over the last `n` trees.
    Call(u8, u8),
}

/// A tree in postfix order.
#[derive(Arbitrary, Debug)]
pub struct Tree(pub Vec<Node>);

impl Tree {
    /// The last tree the nodes leave, if any.
    pub fn expr(&self) -> Option<Expr> {
        let mut stack: Vec<Expr> = Vec::new();
        for node in &self.0 {
            let expr = match node {
                Node::Bool(b) => Expr::Bool(*b),
                Node::Number(n) => Expr::Number(*n),
                Node::String(s) => Expr::String(s.clone()),
                Node::Property(i) => Expr::property(NAMES[usize::from(*i) % NAMES.len()]),
                Node::Date(date) => Expr::Date { date: date.clone() },
                Node::Timestamp(timestamp) => Expr::Timestamp {
                    timestamp: timestamp.clone(),
                },
                Node::List(n) => Expr::List(take(&mut stack, *n)),
                Node::Call(op, n) => {
                    Expr::call(OPS[usize::from(*op) % OPS.len()], take(&mut stack, *n))
                }
            };
            if expr.depth() <= MAX_EXPRESSION_DEPTH {
                stack.push(expr);
            }
        }
        stack.pop()
    }
}

fn take(stack: &mut Vec<Expr>, n: u8) -> Vec<Expr> {
    let at = stack.len().saturating_sub(usize::from(n % 4));
    stack.split_off(at)
}

/// A property's value.
#[derive(Arbitrary, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
}

/// The properties of a feature, by the names of [`NAMES`] in turn.
pub fn properties(values: &[Value]) -> Properties {
    Properties::list(
        values
            .iter()
            .zip(NAMES.iter().cycle())
            .map(|(value, name)| {
                let stored = match value {
                    Value::Null => Stored::Null,
                    Value::Bool(b) => Stored::Bool(*b),
                    Value::Number(text) => Stored::Number(text.as_str().into()),
                    Value::String(text) => Stored::String(text.as_str().into()),
                };
                ((*name).into(), stored)
            })
            .collect(),
    )
}
