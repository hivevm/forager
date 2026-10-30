//! The evaluator on arbitrary trees, patterns, and properties: every operator and function on
//! values of any type, with any `f64` and any text, read as a condition, a number, and a text
//! (Q-3, ADR-0031). A pattern of a tree is a string of the fuzzed bytes, so that `regexp_like`
//! reads arbitrary patterns, each bounded in size (Q-9).
// Verifies: Q-3, ADR-0031
#![no_main]

mod expr_input;

use arbitrary::{Arbitrary, Unstructured};
use forager_expr::Evaluator;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    tree: expr_input::Tree,
    features: Vec<Vec<expr_input::Value>>,
}

/// The engine's lookup, by the source's exact names first and then without regard to case.
struct Lookup(forager_ports::Properties);

impl forager_expr::Lookup for Lookup {
    fn text(&self, name: &str) -> Option<std::borrow::Cow<'_, str>> {
        match self.0.get(name)? {
            forager_ports::Value::Null => None,
            forager_ports::Value::Bool(b) => Some(if b { "1" } else { "0" }.into()),
            forager_ports::Value::Number(text) => Some(text.into()),
            forager_ports::Value::String(text) => Some(text),
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(input) = Input::arbitrary_take_rest(Unstructured::new(data)) else {
        return;
    };
    let Some(expr) = input.tree.expr() else {
        return;
    };
    let mut evaluator = Evaluator::new();
    for values in input.features.iter().take(8) {
        let feature = Lookup(expr_input::properties(values));
        let _ = evaluator.matches(&expr, &feature);
        let _ = evaluator.number(&expr, &feature);
        let _ = evaluator.text(&expr, &feature);
    }
});
