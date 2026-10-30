//! Each operator and function against values worked from MapServer's source (ADR-0031).

use std::collections::BTreeMap;

use super::*;

/// A feature's properties by exact name.
struct Props(BTreeMap<&'static str, &'static str>);

impl Lookup for Props {
    fn text(&self, name: &str) -> Option<Cow<'_, str>> {
        self.0.get(name).map(|v| Cow::Borrowed(*v))
    }
}

fn props(pairs: &[(&'static str, &'static str)]) -> Props {
    Props(pairs.iter().copied().collect())
}

fn call(op: Op, args: Vec<Expr>) -> Expr {
    Expr::call(op, args)
}

fn num(n: f64) -> Expr {
    Expr::Number(n)
}

fn s(text: &str) -> Expr {
    Expr::String(text.to_owned())
}

fn prop_num(name: &str) -> Expr {
    call(Op::ToNumber, vec![Expr::property(name)])
}

fn prop_str(name: &str) -> Expr {
    call(Op::ToString, vec![Expr::property(name)])
}

fn eval(expr: &Expr, feature: &Props) -> Result<bool, Failure> {
    Evaluator::new().matches(expr, feature)
}

fn number(expr: &Expr) -> Result<f64, Failure> {
    Evaluator::new().number(expr, &props(&[]))
}

fn text(expr: &Expr) -> String {
    String::from_utf8(Evaluator::new().text(expr, &props(&[])).unwrap()).unwrap()
}

#[test]
fn comparisons_compare_numbers_as_doubles_and_strings_as_strcmp() {
    // Verifies: ADR-0031
    let feature = props(&[("a", "12abc"), ("b", "B"), ("c", "b"), ("e", "\u{e9}")]);
    let yes = |e: Expr| assert!(eval(&e, &feature).unwrap(), "{e:?}");
    let no = |e: Expr| assert!(!eval(&e, &feature).unwrap(), "{e:?}");
    yes(call(Op::Eq, vec![prop_num("a"), num(12.0)]));
    yes(call(Op::Lt, vec![prop_str("b"), prop_str("c")]));
    no(call(Op::Eq, vec![prop_str("b"), prop_str("c")]));
    // strcmp orders bytes unsigned: é (0xC3 0xA9) after every ASCII letter.
    yes(call(Op::Gt, vec![prop_str("e"), s("z")]));
    yes(call(
        Op::Eq,
        vec![
            call(Op::CaseI, vec![prop_str("b")]),
            call(Op::CaseI, vec![prop_str("c")]),
        ],
    ));
    // casei is ASCII only, as strcasecmp in the C locale.
    no(call(
        Op::Eq,
        vec![
            call(Op::CaseI, vec![s("\u{c9}")]),
            call(Op::CaseI, vec![s("\u{e9}")]),
        ],
    ));
    yes(call(Op::Ge, vec![num(2.0), num(2.0)]));
    yes(call(Op::Ne, vec![num(f64::NAN), num(f64::NAN)]));
    no(call(Op::Eq, vec![num(f64::NAN), num(f64::NAN)]));
    // A property the feature lacks reads as the empty text, and as 0.
    yes(call(Op::Eq, vec![prop_str("missing"), s("")]));
    yes(call(Op::Eq, vec![prop_num("missing"), num(0.0)]));
    // Operands of two types are no production of the grammar.
    assert_eq!(
        eval(&call(Op::Eq, vec![num(1.0), s("1")]), &feature),
        Err(Failure::Undefined)
    );
}

#[test]
fn in_compares_each_item_untrimmed() {
    // Verifies: ADR-0031
    let feature = props(&[("a", "b"), ("n", "2.0")]);
    let list = |items: &[&str]| Expr::List(items.iter().map(|i| s(i)).collect());
    let is_in = |value: Expr, items: Expr| eval(&call(Op::In, vec![value, items]), &feature);
    assert_eq!(is_in(prop_str("a"), list(&["a", "b"])), Ok(true));
    assert_eq!(is_in(prop_str("a"), list(&["a", " b"])), Ok(false));
    assert_eq!(
        is_in(prop_num("n"), Expr::List(vec![num(1.0), num(2.0)])),
        Ok(true)
    );
}

#[test]
fn arithmetic_follows_the_grammar() {
    // Verifies: ADR-0031
    assert_eq!(number(&call(Op::Mod, vec![num(7.9), num(2.5)])), Ok(1.0));
    assert_eq!(number(&call(Op::Mod, vec![num(-7.0), num(2.0)])), Ok(-1.0));
    assert_eq!(
        number(&call(Op::Pow, vec![num(2.0), num(10.0)])),
        Ok(1024.0)
    );
    assert_eq!(number(&call(Op::Div, vec![num(1.0), num(4.0)])), Ok(0.25));
    // By zero, and past an int, the result is undefined: the expression fails.
    assert_eq!(
        number(&call(Op::Div, vec![num(1.0), num(0.0)])),
        Err(Failure::Undefined)
    );
    assert_eq!(
        number(&call(Op::Mod, vec![num(1.0), num(0.5)])),
        Err(Failure::Undefined)
    );
    assert_eq!(
        number(&call(Op::Mod, vec![num(3e9), num(2.0)])),
        Err(Failure::Undefined)
    );
}

#[test]
fn a_failure_anywhere_fails_the_whole_expression() {
    // Verifies: ADR-0031
    // `false AND 1/0 <> 0`: the grammar evaluates both sides before AND.
    let expr = call(
        Op::Or,
        vec![
            Expr::Bool(true),
            call(
                Op::Ne,
                vec![call(Op::Div, vec![num(1.0), num(0.0)]), num(0.0)],
            ),
        ],
    );
    assert_eq!(eval(&expr, &props(&[])), Err(Failure::Undefined));
}

#[test]
fn the_functions_follow_mapserver() {
    // Verifies: ADR-0031
    assert_eq!(number(&call(Op::Length, vec![s("\u{e9}t\u{e9}")])), Ok(5.0));
    assert_eq!(number(&call(Op::Round, vec![num(2.5)])), Ok(2.0));
    assert_eq!(number(&call(Op::Round, vec![num(3.5)])), Ok(4.0));
    assert_eq!(number(&call(Op::Round, vec![num(-2.5)])), Ok(-2.0));
    assert_eq!(
        number(&call(Op::Round, vec![num(25.0), num(10.0)])),
        Ok(20.0)
    );
    assert_eq!(
        number(&call(Op::Round, vec![num(35.0), num(10.0)])),
        Ok(40.0)
    );
    assert_eq!(
        number(&call(Op::Round, vec![num(1.0), num(0.0)])),
        Err(Failure::Undefined)
    );
    assert_eq!(text(&call(Op::Upper, vec![s("r\u{e9}d")])), "R\u{e9}D");
    assert_eq!(text(&call(Op::Lower, vec![s("R\u{c9}D")])), "r\u{c9}d");
    assert_eq!(
        text(&call(Op::Initcap, vec![s("hELLO wORLD")])),
        "Hello World"
    );
    assert_eq!(
        text(&call(Op::Firstcap, vec![s("hELLO wORLD")])),
        "Hello world"
    );
    assert_eq!(
        text(&call(Op::Commify, vec![s("1234567.891")])),
        "1,234,567.891"
    );
    assert_eq!(text(&call(Op::Concat, vec![s("a"), s("b"), s("c")])), "abc");
    assert_eq!(
        text(&call(Op::Tostring, vec![num(1.23456), s("%.2f")])),
        "1.23"
    );
    assert_eq!(
        Evaluator::new().text(&call(Op::Tostring, vec![num(1.0), s("%d")]), &props(&[])),
        Err(Failure::Undefined)
    );
    assert_eq!(text(&call(Op::ToString, vec![Expr::Bool(true)])), "true");
    assert_eq!(text(&call(Op::ToString, vec![num(0.1 + 0.2)])), "0.3");
    assert_eq!(text(&call(Op::ToString, vec![num(1e20)])), "1e+20");
    assert_eq!(
        number(&call(
            Op::ToNumber,
            vec![call(Op::Tostring, vec![num(1234567.0), s("%g")])]
        )),
        Ok(1234570.0)
    );
}

#[test]
fn a_unary_minus_is_carried_as_its_operand() {
    // Verifies: ADR-0031
    // The frontend writes `-[a]` as `to_number(a)`, as mapparser.y's `$$ = $2` does not negate;
    // the evaluator holds no negation that could apply.
    let feature = props(&[("a", "5")]);
    assert_eq!(Evaluator::new().number(&prop_num("a"), &feature), Ok(5.0));
}

#[test]
fn times_compare_as_ms_time_compare() {
    // Verifies: ADR-0031
    let feature = props(&[("t", "2010-12-01T10:00:00"), ("bad", "yesterday")]);
    let at = |text: &str| Expr::Timestamp {
        timestamp: text.to_owned(),
    };
    let t = call(Op::ToTimestamp, vec![Expr::property("t")]);
    assert_eq!(
        eval(
            &call(Op::TBefore, vec![t.clone(), at("2010-12-01T10:00:01Z")]),
            &feature
        ),
        Ok(true)
    );
    let day = Expr::Date {
        date: "2010-12-01".to_owned(),
    };
    assert_eq!(
        eval(&call(Op::TAfter, vec![t.clone(), day]), &feature),
        Ok(true)
    );
    assert_eq!(
        eval(&call(Op::TEquals, vec![t.clone(), t]), &feature),
        Ok(true)
    );
    let bad = call(Op::ToTimestamp, vec![Expr::property("bad")]);
    assert_eq!(
        eval(&call(Op::TEquals, vec![bad.clone(), bad]), &feature),
        Err(Failure::Undefined)
    );
}

fn like(value: Expr, pattern: &str, flags: &str) -> Expr {
    call(Op::RegexpLike, vec![value, s(pattern), s(flags)])
}

#[test]
fn an_empty_value_never_matches_a_pattern() {
    // Verifies: ADR-0031
    let feature = props(&[("a", "")]);
    assert_eq!(eval(&like(prop_str("a"), "^$", ""), &feature), Ok(false));
    assert_eq!(eval(&like(prop_str("a"), ".*", ""), &feature), Ok(false));
    // Not even one PCRE2 reads and the crate cannot: the value is tested first.
    assert_eq!(eval(&like(prop_str("a"), "(?=a)", ""), &feature), Ok(false));
}

#[test]
fn a_pattern_pcre2_refuses_is_false_where_it_stands() {
    // Verifies: ADR-0031
    let feature = props(&[("a", "x")]);
    let refused = like(prop_str("a"), "(", "");
    assert_eq!(eval(&refused, &feature), Ok(false));
    assert_eq!(eval(&call(Op::Not, vec![refused]), &feature), Ok(true));
}

#[test]
fn the_flag_i_and_the_classes_are_ascii() {
    // Verifies: ADR-0031
    let feature = props(&[("a", "ROAD"), ("digit", "\u{663}"), ("e", "\u{c9}")]);
    assert_eq!(
        eval(&like(prop_str("a"), "^road$", "i"), &feature),
        Ok(true)
    );
    assert_eq!(
        eval(&like(prop_str("a"), "^road$", ""), &feature),
        Ok(false)
    );
    assert_eq!(
        eval(&like(prop_str("digit"), r"\d", ""), &feature),
        Ok(false)
    );
    assert_eq!(
        eval(&like(prop_str("e"), "^\u{e9}$", "i"), &feature),
        Ok(false)
    );
    assert_eq!(eval(&like(prop_str("e"), "^..$", ""), &feature), Ok(true));
}

#[test]
fn a_pattern_past_the_size_limit_does_not_match() {
    // Verifies: ADR-0031, Q-9
    let feature = props(&[("a", "x")]);
    let result = eval(
        &like(prop_str("a"), r"\w{1000}[a-z]{1000}x{60000}", ""),
        &feature,
    );
    assert!(matches!(result, Err(Failure::Pattern { .. })), "{result:?}");
}

#[test]
fn unsupported_constructs_fail_the_expression_for_every_feature() {
    // Verifies: ADR-0031
    let mut evaluator = Evaluator::new();
    for pattern in ["a*+", r"(a)\1"] {
        let expr = call(Op::Not, vec![like(prop_str("a"), pattern, "")]);
        for value in ["aa", "b"] {
            let feature = props(&[("a", value)]);
            let result = evaluator.matches(&expr, &feature);
            assert!(
                matches!(&result, Err(Failure::Pattern { pattern: p, .. }) if p == pattern),
                "{pattern} on {value}: {result:?}"
            );
        }
    }
}

#[test]
fn conditions_and_whole_values_read_as_msevalexpression_takes_them() {
    let feature = props(&[]);
    assert_eq!(eval(&num(2.0), &feature), Ok(true));
    assert_eq!(eval(&num(0.0), &feature), Ok(false));
    assert_eq!(eval(&s(""), &feature), Ok(true));
    assert_eq!(number(&s("3.5px")), Ok(3.5));
    assert_eq!(number(&Expr::Bool(true)), Ok(0.0));
}

#[test]
fn a_tree_past_the_depth_limit_fails() {
    let mut expr = Expr::Bool(true);
    for _ in 0..MAX_EXPRESSION_DEPTH {
        expr = call(Op::Not, vec![expr]);
    }
    assert_eq!(eval(&expr, &props(&[])), Err(Failure::Undefined));
}

#[test]
fn a_nul_ends_a_string_as_in_c() {
    let feature = props(&[("a", "ab\0cd")]);
    assert_eq!(
        eval(&call(Op::Eq, vec![prop_str("a"), s("ab")]), &feature),
        Ok(true)
    );
}

#[test]
fn only_patterns_written_in_the_tree_are_kept() {
    // Verifies: ADR-0031, Q-9
    let mut evaluator = Evaluator::new();
    let literal = like(prop_str("a"), "^x", "");
    let computed = call(Op::RegexpLike, vec![prop_str("a"), prop_str("p"), s("")]);
    for pattern in ["^a", "^b", "^c"] {
        let feature = Props([("a", "abc"), ("p", pattern)].into_iter().collect());
        evaluator.matches(&literal, &feature).unwrap();
        evaluator.matches(&computed, &feature).unwrap();
    }
    assert_eq!(evaluator.cached(), 1);
    let feature = props(&[("a", "abc"), ("p", "^a")]);
    assert_eq!(evaluator.matches(&computed, &feature), Ok(true));
}

#[test]
fn a_text_past_the_limit_fails_the_expression() {
    // Verifies: Q-9
    let long: &'static str = Box::leak("x".repeat(MAX_TEXT_BYTES / 4 + 1).into_boxed_str());
    let feature = props(&[("a", long)]);
    let four = call(
        Op::Concat,
        vec![prop_str("a"), prop_str("a"), prop_str("a"), prop_str("a")],
    );
    let expr = call(Op::Eq, vec![four, s("")]);
    assert_eq!(eval(&expr, &feature), Err(Failure::TooLong));
    let three = call(
        Op::Concat,
        vec![prop_str("a"), prop_str("a"), prop_str("a")],
    );
    assert_eq!(eval(&call(Op::Eq, vec![three, s("")]), &feature), Ok(false));
}

#[test]
fn a_computed_pattern_past_the_length_bound_does_not_match() {
    // Verifies: Q-9
    let pattern: &'static str = Box::leak("a".repeat(MAX_PATTERN_BYTES + 1).into_boxed_str());
    let feature = props(&[("a", "a"), ("p", pattern)]);
    let computed = call(Op::RegexpLike, vec![prop_str("a"), prop_str("p"), s("")]);
    let result = Evaluator::new().matches(&computed, &feature);
    assert!(matches!(result, Err(Failure::Pattern { .. })), "{result:?}");
}
