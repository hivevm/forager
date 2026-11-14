//! The transform tree: a `GEOMTRANSFORM` expression of a layer or of a class's style as a typed
//! tree of its shape functions, serialized in the shape of CQL2-JSON (ADR-0036).
//! The mapfile frontend alone translates MapServer's text into it; the tree holds no text of the
//! mapfile, and a transform the engine does not draw is the whole tree `{"not_drawn": reason}`.

use std::fmt;

use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A `GEOMTRANSFORM` expression: a tree of the functions of a shape the engine draws, or the
/// reason it does not draw the transform.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformTree {
    Drawn(TransformShape),
    NotDrawn(NotDrawn),
}

/// A shape the tree computes: the feature's shape, or a function of a shape.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformShape {
    /// The feature's shape, MapServer's `[shape]`.
    Shape,
    /// `generalize(shape, tolerance)`.
    Generalize(Box<TransformShape>, TransformNumber),
    /// `densify(shape, tolerance)`.
    Densify(Box<TransformShape>, TransformNumber),
    /// `outer(shape)`: the outer rings of a polygon.
    Outer(Box<TransformShape>),
    /// `inner(shape)`: the inner rings of a polygon.
    Inner(Box<TransformShape>),
    /// `smoothsia(shape, size, iterations[, preprocessing])`; MapServer's grammar gives a size of
    /// 3 and 1 iteration where they are not written, and a reader of a tree without them gives
    /// the same.
    SmoothSia {
        shape: Box<TransformShape>,
        size: TransformNumber,
        iterations: TransformNumber,
        preprocessing: Option<Preprocessing>,
    },
}

/// A number argument: a number, the map's cellsize in the layer's units, or an operation of two
/// numbers. The frontend folds every operation without `map_cellsize` to one number at load.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformNumber {
    Number(f64),
    /// MapServer's `[map_cellsize]`.
    MapCellsize,
    Op(NumberOp, Box<TransformNumber>, Box<TransformNumber>),
}

/// The operators of a number argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum NumberOp {
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "-")]
    Sub,
    #[serde(rename = "*")]
    Mul,
    #[serde(rename = "/")]
    Div,
    /// The remainder of the operands cut to C's `int`s, MapServer's `(int)$1 % (int)$3`.
    #[serde(rename = "%")]
    Mod,
    #[serde(rename = "^")]
    Pow,
}

/// The preprocessing of `smoothsia`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Preprocessing {
    /// Midpoints added about every point whose window is whole.
    All,
    /// Midpoints added about every point whose window's path turns.
    Angle,
}

/// Why the engine does not draw a transform: the first construct of the expression, in its
/// reading order, the tree has no node for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum NotDrawn {
    Buffer,
    Centerline,
    Difference,
    Simplify,
    Simplifypt,
    Javascript,
    Length,
    Area,
    Round,
    /// A property's value where a number or a text is due.
    Binding,
    /// A computed `smoothsia` preprocessing.
    Text,
    /// A shape `fromtext` reads from a text.
    Fromtext,
    /// An operation of literals that folds to no finite JSON number, a `%` by 0 among them.
    Literal,
    /// An expression MapServer's grammar does not parse as a shape, a `/` by 0 among them.
    Syntax,
}

impl NotDrawn {
    /// The reason's name, as the tree writes it.
    pub fn name(self) -> &'static str {
        match self {
            NotDrawn::Buffer => "buffer",
            NotDrawn::Centerline => "centerline",
            NotDrawn::Difference => "difference",
            NotDrawn::Simplify => "simplify",
            NotDrawn::Simplifypt => "simplifypt",
            NotDrawn::Javascript => "javascript",
            NotDrawn::Length => "length",
            NotDrawn::Area => "area",
            NotDrawn::Round => "round",
            NotDrawn::Binding => "binding",
            NotDrawn::Text => "text",
            NotDrawn::Fromtext => "fromtext",
            NotDrawn::Literal => "literal",
            NotDrawn::Syntax => "syntax",
        }
    }
}

/// Whether `op` names one of the tree's functions: a function of a shape, or `map_cellsize`,
/// which a reader of a newer minor's transform tree knows as operators of the IR.
pub(crate) fn is_function(op: &str) -> bool {
    SHAPE_FUNCTIONS.contains(&op) || op == "map_cellsize"
}

/// A node of the tree, for its depth.
#[derive(Clone, Copy)]
enum Node<'a> {
    Shape(&'a TransformShape),
    Number(&'a TransformNumber),
    Leaf,
}

impl TransformTree {
    /// How deep the tree nests, counted as [`crate::Expr::depth`] counts an expression: a `shape`,
    /// a number, a preprocessing, or a `not_drawn` is 1. Iterative, so that measuring a tree too
    /// deep to accept cannot overflow the stack.
    pub fn depth(&self) -> usize {
        let TransformTree::Drawn(root) = self else {
            return 1;
        };
        let mut deepest = 0;
        let mut pending = vec![(Node::Shape(root), 1)];
        while let Some((node, depth)) = pending.pop() {
            deepest = deepest.max(depth);
            let below = depth + 1;
            match node {
                Node::Shape(TransformShape::Shape) | Node::Leaf => {}
                Node::Shape(
                    TransformShape::Generalize(shape, number)
                    | TransformShape::Densify(shape, number),
                ) => {
                    pending.push((Node::Shape(shape), below));
                    pending.push((Node::Number(number), below));
                }
                Node::Shape(TransformShape::Outer(shape) | TransformShape::Inner(shape)) => {
                    pending.push((Node::Shape(shape), below));
                }
                Node::Shape(TransformShape::SmoothSia {
                    shape,
                    size,
                    iterations,
                    preprocessing,
                }) => {
                    pending.push((Node::Shape(shape), below));
                    pending.push((Node::Number(size), below));
                    pending.push((Node::Number(iterations), below));
                    if preprocessing.is_some() {
                        pending.push((Node::Leaf, below));
                    }
                }
                Node::Number(TransformNumber::Number(_)) => {}
                Node::Number(TransformNumber::MapCellsize) => {}
                Node::Number(TransformNumber::Op(_, left, right)) => {
                    pending.push((Node::Number(left), below));
                    pending.push((Node::Number(right), below));
                }
            }
        }
        deepest
    }
}

/// An argument of a node as it is written.
enum Arg<'a> {
    Shape(&'a TransformShape),
    Number(&'a TransformNumber),
    Preprocessing(Preprocessing),
}

impl Serialize for Arg<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Arg::Shape(shape) => shape.serialize(serializer),
            Arg::Number(number) => number.serialize(serializer),
            Arg::Preprocessing(preprocessing) => preprocessing.serialize(serializer),
        }
    }
}

/// `{"op": op, "args": args}`.
fn call<S: Serializer>(serializer: S, op: &str, args: &[Arg<'_>]) -> Result<S::Ok, S::Error> {
    struct Args<'a, 'b>(&'a [Arg<'b>]);
    impl Serialize for Args<'_, '_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
            for arg in self.0 {
                seq.serialize_element(arg)?;
            }
            seq.end()
        }
    }
    let mut map = serializer.serialize_map(Some(2))?;
    map.serialize_entry("op", op)?;
    map.serialize_entry("args", &Args(args))?;
    map.end()
}

impl Serialize for TransformTree {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TransformTree::Drawn(shape) => shape.serialize(serializer),
            TransformTree::NotDrawn(reason) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("not_drawn", reason)?;
                map.end()
            }
        }
    }
}

impl Serialize for TransformShape {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TransformShape::Shape => call(serializer, "shape", &[]),
            TransformShape::Generalize(shape, number) => call(
                serializer,
                "generalize",
                &[Arg::Shape(shape), Arg::Number(number)],
            ),
            TransformShape::Densify(shape, number) => call(
                serializer,
                "densify",
                &[Arg::Shape(shape), Arg::Number(number)],
            ),
            TransformShape::Outer(shape) => call(serializer, "outer", &[Arg::Shape(shape)]),
            TransformShape::Inner(shape) => call(serializer, "inner", &[Arg::Shape(shape)]),
            TransformShape::SmoothSia {
                shape,
                size,
                iterations,
                preprocessing,
            } => {
                let mut args = vec![
                    Arg::Shape(shape),
                    Arg::Number(size),
                    Arg::Number(iterations),
                ];
                args.extend(preprocessing.map(Arg::Preprocessing));
                call(serializer, "smoothsia", &args)
            }
        }
    }
}

impl Serialize for TransformNumber {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TransformNumber::Number(n) => serializer.serialize_f64(*n),
            TransformNumber::MapCellsize => call(serializer, "map_cellsize", &[]),
            TransformNumber::Op(op, left, right) => {
                let name = match op {
                    NumberOp::Add => "+",
                    NumberOp::Sub => "-",
                    NumberOp::Mul => "*",
                    NumberOp::Div => "/",
                    NumberOp::Mod => "%",
                    NumberOp::Pow => "^",
                };
                call(serializer, name, &[Arg::Number(left), Arg::Number(right)])
            }
        }
    }
}

/// A node as the reader meets it, before it is typed by its place: a number, a text, a call,
/// or a `not_drawn`. Its keys are read one by one, and a key it does not know is read as
/// ignored, so that the style document's reader names it with its path (ADR-0014).
enum Read {
    Number(f64),
    Text(String),
    Call { op: String, args: Vec<Read> },
    NotDrawn(NotDrawn),
}

struct ReadVisitor;

impl<'de> Visitor<'de> for ReadVisitor {
    type Value = Read;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a node of a transform tree")
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Read, E> {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a JSON integer is read as the double a number is"
        )]
        Ok(Read::Number(v as f64))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Read, E> {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a JSON integer is read as the double a number is"
        )]
        Ok(Read::Number(v as f64))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Read, E> {
        Ok(Read::Number(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Read, E> {
        Ok(Read::Text(v.to_owned()))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Read, A::Error> {
        let mut op: Option<String> = None;
        let mut args: Option<Vec<Read>> = None;
        let mut not_drawn: Option<NotDrawn> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "op" if op.is_none() => op = Some(map.next_value()?),
                "args" if args.is_none() => args = Some(map.next_value::<Args>()?.0),
                "not_drawn" if not_drawn.is_none() => not_drawn = Some(map.next_value()?),
                "op" | "args" | "not_drawn" => {
                    return Err(de::Error::custom(format!("duplicate field `{key}`")));
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        match (op, args, not_drawn) {
            (Some(op), Some(args), None) => Ok(Read::Call { op, args }),
            (None, None, Some(reason)) => Ok(Read::NotDrawn(reason)),
            _ => Err(de::Error::custom(
                "a node of a transform tree is `{\"op\", \"args\"}` or `{\"not_drawn\"}`",
            )),
        }
    }
}

/// The arguments of a call, each read as a node.
struct Args(Vec<Read>);

impl<'de> Deserialize<'de> for Args {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArgsVisitor;
        impl<'de> Visitor<'de> for ArgsVisitor {
            type Value = Args;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("the arguments of a call")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Args, A::Error> {
                let mut args = Vec::new();
                while let Some(arg) = seq.next_element::<Read>()? {
                    args.push(arg);
                }
                Ok(Args(args))
            }
        }
        deserializer.deserialize_seq(ArgsVisitor)
    }
}

impl<'de> Deserialize<'de> for Read {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ReadVisitor)
    }
}

/// The functions of a shape, the values of `op` where a shape is due.
const SHAPE_FUNCTIONS: &[&str] = &[
    "shape",
    "generalize",
    "densify",
    "outer",
    "inner",
    "smoothsia",
];

/// The values of `op` where a number is due.
const NUMBER_FUNCTIONS: &[&str] = &["map_cellsize", "+", "-", "*", "/", "%", "^"];

fn arity<E: de::Error>(op: &str, expected: &str) -> E {
    E::custom(format!("`{op}` of a transform tree takes {expected}"))
}

fn shape<E: de::Error>(node: Read) -> Result<TransformShape, E> {
    let Read::Call { op, args } = node else {
        return Err(E::custom(
            "a shape of a transform tree is `{\"op\", \"args\"}`",
        ));
    };
    let mut args = args.into_iter();
    let shape_arg = |args: &mut std::vec::IntoIter<Read>| {
        args.next()
            .map(shape::<E>)
            .transpose()?
            .map(Box::new)
            .ok_or_else(|| arity::<E>(&op, "a shape first"))
    };
    let transformed = match op.as_str() {
        "shape" => TransformShape::Shape,
        "generalize" | "densify" => {
            let inner = shape_arg(&mut args)?;
            let tolerance = args
                .next()
                .map(number::<E>)
                .transpose()?
                .ok_or_else(|| arity::<E>(&op, "a shape and a number"))?;
            if op == "generalize" {
                TransformShape::Generalize(inner, tolerance)
            } else {
                TransformShape::Densify(inner, tolerance)
            }
        }
        "outer" => TransformShape::Outer(shape_arg(&mut args)?),
        "inner" => TransformShape::Inner(shape_arg(&mut args)?),
        "smoothsia" => {
            let inner = shape_arg(&mut args)?;
            let size = args.next().map(number::<E>).transpose()?;
            let iterations = args.next().map(number::<E>).transpose()?;
            let preprocessing = match args.next() {
                None => None,
                Some(Read::Text(text)) => Some(Preprocessing::deserialize(
                    de::value::StrDeserializer::<E>::new(&text),
                )?),
                Some(_) => return Err(arity(&op, "a preprocessing of `all` or `angle` last")),
            };
            TransformShape::SmoothSia {
                shape: inner,
                size: size.unwrap_or(TransformNumber::Number(3.0)),
                iterations: iterations.unwrap_or(TransformNumber::Number(1.0)),
                preprocessing,
            }
        }
        other => return Err(E::unknown_variant(other, SHAPE_FUNCTIONS)),
    };
    if args.next().is_some() {
        return Err(arity(&op, "no further argument"));
    }
    Ok(transformed)
}

fn number<E: de::Error>(node: Read) -> Result<TransformNumber, E> {
    let (op, args) = match node {
        Read::Number(n) => return Ok(TransformNumber::Number(n)),
        Read::Call { op, args } => (op, args),
        Read::Text(_) | Read::NotDrawn(_) => {
            return Err(E::custom(
                "a number of a transform tree is a number or `{\"op\", \"args\"}`",
            ));
        }
    };
    let operator = match op.as_str() {
        "map_cellsize" if args.is_empty() => return Ok(TransformNumber::MapCellsize),
        "map_cellsize" => return Err(arity(&op, "no argument")),
        "+" => NumberOp::Add,
        "-" => NumberOp::Sub,
        "*" => NumberOp::Mul,
        "/" => NumberOp::Div,
        "%" => NumberOp::Mod,
        "^" => NumberOp::Pow,
        other => return Err(E::unknown_variant(other, NUMBER_FUNCTIONS)),
    };
    let [left, right]: [Read; 2] = args
        .try_into()
        .map_err(|_| arity::<E>(&op, "two numbers"))?;
    Ok(TransformNumber::Op(
        operator,
        Box::new(number(left)?),
        Box::new(number(right)?),
    ))
}

impl<'de> Deserialize<'de> for TransformTree {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Read::deserialize(deserializer)? {
            Read::NotDrawn(reason) => Ok(TransformTree::NotDrawn(reason)),
            node => shape(node).map(TransformTree::Drawn),
        }
    }
}

#[cfg(feature = "schema")]
mod schema {
    use std::borrow::Cow;

    use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

    use super::{
        NotDrawn, NumberOp, Preprocessing, TransformNumber, TransformShape, TransformTree,
    };

    /// `{"op": op, "args": args}`, its `op` one of `ops`, its `args` the schemas `args` in order,
    /// of which the first `required` are written.
    fn call(ops: &[&str], args: &[Schema], required: usize) -> Schema {
        json_schema!({
            "type": "object",
            "properties": {
                "op": { "type": "string", "enum": ops },
                "args": {
                    "type": "array",
                    "prefixItems": args,
                    "items": false,
                    "minItems": required,
                },
            },
            "required": ["op", "args"],
        })
    }

    impl JsonSchema for TransformTree {
        fn schema_name() -> Cow<'static, str> {
            "TransformTree".into()
        }

        fn json_schema(generator: &mut SchemaGenerator) -> Schema {
            let shape = generator.subschema_for::<TransformShape>();
            let reason = generator.subschema_for::<NotDrawn>();
            json_schema!({
                "description": "A `GEOMTRANSFORM` expression: a tree of the functions of a shape \
                                the engine draws, or the reason it does not draw the transform.",
                "oneOf": [
                    shape,
                    {
                        "type": "object",
                        "properties": { "not_drawn": reason },
                        "required": ["not_drawn"],
                    },
                ],
            })
        }
    }

    impl JsonSchema for TransformShape {
        fn schema_name() -> Cow<'static, str> {
            "TransformShape".into()
        }

        fn json_schema(generator: &mut SchemaGenerator) -> Schema {
            let shape = generator.subschema_for::<TransformShape>();
            let number = generator.subschema_for::<TransformNumber>();
            let preprocessing = generator.subschema_for::<Preprocessing>();
            json_schema!({
                "description": "A shape the tree computes: the feature's shape, or a function of \
                                a shape.",
                "oneOf": [
                    call(&["shape"], &[], 0),
                    call(&["generalize", "densify"], &[shape.clone(), number.clone()], 2),
                    call(&["outer", "inner"], std::slice::from_ref(&shape), 1),
                    call(&["smoothsia"], &[shape, number.clone(), number, preprocessing], 1),
                ],
            })
        }
    }

    impl JsonSchema for TransformNumber {
        fn schema_name() -> Cow<'static, str> {
            "TransformNumber".into()
        }

        fn json_schema(generator: &mut SchemaGenerator) -> Schema {
            let number = generator.subschema_for::<TransformNumber>();
            let op = generator.subschema_for::<NumberOp>();
            json_schema!({
                "description": "A number argument: a number, the map's cellsize in the layer's \
                                units, or an operation of two numbers.",
                "oneOf": [
                    { "type": "number" },
                    call(&["map_cellsize"], &[], 0),
                    {
                        "type": "object",
                        "properties": {
                            "op": op,
                            "args": {
                                "type": "array",
                                "prefixItems": [number.clone(), number],
                                "items": false,
                                "minItems": 2,
                            },
                        },
                        "required": ["op", "args"],
                    },
                ],
            })
        }
    }
}

#[cfg(test)]
mod tests;
