use rand::RngExt;
use serde::{Deserialize, Deserializer, Serialize};

/// Operator for a [`Value::Op`] block — arithmetic, [`Op::Random`], or
/// [`Op::Join`] (text concatenation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
    /// `args[0]`/`args[1]` are the inclusive from/to bounds, not operands to
    /// combine — evaluated fresh on every call to `eval`, unlike the
    /// arithmetic ops which are pure functions of their operands. Picks an
    /// integer when both bounds evaluate to whole numbers, otherwise a
    /// float — see `eval`'s `Op::Random` arm.
    Random,
    /// Concatenates all of `args` as text — the only variable-arity
    /// operator today (2 or 3 args, depending on which palette entry it was
    /// dropped from; see `OPERATOR_KINDS`'s `"Join"`/`"Join3"` rows).
    Join,
}

/// A small recursive expression tree backing a numeric instruction field —
/// either a number, a piece of text, or an operator applied to its nested
/// `args` (e.g. `5 + 3`, or `join "a" "b"`). Groundwork for a future
/// macro-wide variable system: today only `Instruction::Wait` and the
/// `MoveMouse`/`Scroll` tokens embed one, always evaluated back down to a
/// number via [`Value::eval_number`]; `Instruction`'s `Text` token embeds one
/// evaluated to a string via [`Value::eval_text`].
///
/// `Op::saved` is the value the operator displaced when it took over the
/// slot — not one of the operator's operands, and not addressable via
/// `get_mut`'s path. It just rides along so the UI can restore it verbatim
/// if the operator is later dragged back out of the slot.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub(crate) enum Value {
    Number { value: f64 },
    Text { value: String },
    Op { op: Op, args: Vec<Value>, saved: Box<Value> },
}

/// The result of evaluating a [`Value`] tree — still either a number or
/// text, since a leaf can be either; only an arithmetic/`Random` [`Op`]
/// forces its args down to a number (via [`Evaluated::as_number`]).
pub(crate) enum Evaluated {
    Number(f64),
    Text(String),
}

impl Evaluated {
    /// Coerces the evaluated result to a number, parsing text loosely (so a
    /// `Text` leaf holding `"5"` still works as an operand); fails with a
    /// readable message for non-numeric text.
    pub(crate) fn as_number(&self) -> Result<f64, String> {
        match self {
            Evaluated::Number(n) => Ok(*n),
            Evaluated::Text(s) => s.trim().parse::<f64>().map_err(|_| format!("\"{s}\" is not a number")),
        }
    }
}

/// Static per-palette-entry metadata driving default construction when a
/// block is dropped from the sidebar — the single place a new operator's
/// arity and default argument type need registering. Arity lives on the
/// *kind string*, not on `Op` itself, since `Op::Join` alone doesn't have
/// one true arity (2 vs 3 is just which palette entry it came from).
/// `commands.rs`'s `apply_value_kind` is the only consumer.
pub(crate) struct OperatorKindSpec {
    pub(crate) kind: &'static str,
    pub(crate) op: Op,
    pub(crate) arity: usize,
    pub(crate) default_arg: fn() -> Value,
}

pub(crate) const OPERATOR_KINDS: &[OperatorKindSpec] = &[
    OperatorKindSpec { kind: "Add", op: Op::Add, arity: 2, default_arg: || Value::number(0.0) },
    OperatorKindSpec { kind: "Sub", op: Op::Sub, arity: 2, default_arg: || Value::number(0.0) },
    OperatorKindSpec { kind: "Mul", op: Op::Mul, arity: 2, default_arg: || Value::number(0.0) },
    OperatorKindSpec { kind: "Div", op: Op::Div, arity: 2, default_arg: || Value::number(0.0) },
    OperatorKindSpec { kind: "Random", op: Op::Random, arity: 2, default_arg: || Value::number(0.0) },
    OperatorKindSpec { kind: "Join", op: Op::Join, arity: 2, default_arg: || Value::Text { value: String::new() } },
    OperatorKindSpec { kind: "Join3", op: Op::Join, arity: 3, default_arg: || Value::Text { value: String::new() } },
];

impl Value {
    pub(crate) fn number(value: f64) -> Self {
        Value::Number { value }
    }

    pub(crate) fn eval(&self) -> Result<Evaluated, String> {
        match self {
            Value::Number { value } => Ok(Evaluated::Number(*value)),
            Value::Text { value } => Ok(Evaluated::Text(value.clone())),
            Value::Op { op: Op::Join, args, .. } => {
                let mut s = String::new();
                for a in args {
                    s.push_str(&a.eval_text()?);
                }
                Ok(Evaluated::Text(s))
            }
            Value::Op { op, args, .. } => {
                let l = args[0].eval()?.as_number()?;
                let r = args[1].eval()?.as_number()?;
                let result = match op {
                    Op::Add => l + r,
                    Op::Sub => l - r,
                    Op::Mul => l * r,
                    Op::Div => {
                        if r == 0.0 {
                            return Err("division by zero".to_string());
                        }
                        l / r
                    }
                    Op::Random => {
                        let (lo, hi) = (l.min(r), l.max(r));
                        if l.fract() == 0.0 && r.fract() == 0.0 {
                            rand::rng().random_range(lo as i64..=hi as i64) as f64
                        } else {
                            rand::rng().random_range(lo..=hi)
                        }
                    }
                    Op::Join => unreachable!("Op::Join matched above"),
                };
                Ok(Evaluated::Number(result))
            }
        }
    }

    /// Evaluates the tree and coerces the result to a number — the entry
    /// point every numeric instruction field actually calls.
    pub(crate) fn eval_number(&self) -> Result<f64, String> {
        self.eval()?.as_number()
    }

    /// Evaluates the tree down to a string — the entry point the `Text`
    /// instruction token calls. A `Text` leaf or an `Op::Join` result passes
    /// through verbatim; a `Number` leaf or another `Op` result (always
    /// numeric, even for `Op::Random`) gets stringified.
    pub(crate) fn eval_text(&self) -> Result<String, String> {
        Ok(match self.eval()? {
            Evaluated::Text(s) => s,
            Evaluated::Number(n) => n.to_string(),
        })
    }

    /// Walks `path` (`n` steps into `args[n]` at each `Op`) down to the
    /// addressed node; an empty path returns `self`.
    pub(crate) fn get_mut(&mut self, path: &[u8]) -> Option<&mut Value> {
        match path.split_first() {
            None => Some(self),
            Some((&step, rest)) => match self {
                Value::Op { args, .. } => args.get_mut(step as usize)?.get_mut(rest),
                _ => None,
            },
        }
    }
}

/// Manual impl mirroring `Instruction`'s own manual `Hash` impl
/// (`macros/mod.rs`) — `f64` isn't `Hash`, so its bit pattern stands in.
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Number { value } => {
                0u8.hash(state);
                value.to_bits().hash(state);
            }
            Value::Text { value } => {
                1u8.hash(state);
                value.hash(state);
            }
            Value::Op { op, args, saved } => {
                2u8.hash(state);
                op.hash(state);
                args.hash(state);
                saved.hash(state);
            }
        }
    }
}

/// Accepts either the tagged `{"kind": "...", ...}` shape or a bare number —
/// old save files have plain numbers in the slots `Value` now occupies
/// (e.g. `"MoveMouse":[5,10,"Rel"]`), so this keeps them loading without a
/// wrapper enum at every embedding site, mirroring the `WaitDe`/
/// `InstructionDe` untagged-enum pattern in `macros/mod.rs`.
impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        fn default_saved() -> Box<Value> {
            Box::new(Value::number(0.0))
        }

        #[derive(Deserialize)]
        #[serde(tag = "kind")]
        enum Tagged {
            Number { value: f64 },
            Text { value: String },
            Op {
                op: Op,
                args: Vec<Value>,
                // Older save files predate `saved` entirely — falls back to
                // a plain zero, same spirit as `ValueDe::Legacy` below.
                #[serde(default = "default_saved")]
                saved: Box<Value>,
            },
            // Legacy tags predating the `BinaryOp`/`Join` → `Op` unification
            // — kept only so old save files keep loading; migrated into
            // `Value::Op` below, never produced by `Serialize`.
            BinaryOp {
                op: Op,
                lhs: Box<Value>,
                rhs: Box<Value>,
                #[serde(default = "default_saved")]
                saved: Box<Value>,
            },
            Join {
                args: Vec<Value>,
                #[serde(default = "default_saved")]
                saved: Box<Value>,
            },
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ValueDe {
            Legacy(f64),
            // Pre-`Value` shape of `InputToken::Text`: a bare JSON string,
            // from back when the `Text` instruction just held a `String`
            // rather than a full expression tree.
            LegacyText(String),
            Current(Tagged),
        }

        Ok(match ValueDe::deserialize(deserializer)? {
            ValueDe::Legacy(n) => Value::Number { value: n },
            ValueDe::LegacyText(s) => Value::Text { value: s },
            ValueDe::Current(Tagged::Number { value }) => Value::Number { value },
            ValueDe::Current(Tagged::Text { value }) => Value::Text { value },
            ValueDe::Current(Tagged::Op { op, args, saved }) => Value::Op { op, args, saved },
            ValueDe::Current(Tagged::BinaryOp { op, lhs, rhs, saved }) => Value::Op { op, args: vec![*lhs, *rhs], saved },
            ValueDe::Current(Tagged::Join { args, saved }) => Value::Op { op: Op::Join, args, saved },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_bare_number_deserializes_to_number() {
        let v: Value = serde_json::from_str("5.0").unwrap();
        assert_eq!(v, Value::number(5.0));
    }

    #[test]
    fn legacy_bare_string_deserializes_to_text() {
        // Pre-`Value` save shape of `InputToken::Text`, from back when it
        // held a plain `String` — must keep loading old macros.
        let v: Value = serde_json::from_str("\"hello\"").unwrap();
        assert_eq!(v, Value::Text { value: "hello".into() });
    }

    #[test]
    fn eval_text_passes_through_text_leaf() {
        let v = Value::Text { value: "hi".into() };
        assert_eq!(v.eval_text(), Ok("hi".to_string()));
    }

    #[test]
    fn eval_text_stringifies_numeric_result() {
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::number(2.0), Value::number(3.0)],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_text(), Ok("5".to_string()));
    }

    #[test]
    fn tagged_shape_round_trips() {
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::number(2.0), Value::Text { value: "3".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn legacy_binary_op_tag_migrates_to_op() {
        let json = r#"{"kind":"BinaryOp","op":"Add","lhs":{"kind":"Number","value":2.0},"rhs":{"kind":"Number","value":3.0},"saved":{"kind":"Number","value":0.0}}"#;
        let v: Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            v,
            Value::Op { op: Op::Add, args: vec![Value::number(2.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) }
        );
    }

    #[test]
    fn legacy_join_tag_migrates_to_op() {
        let json = r#"{"kind":"Join","args":[{"kind":"Text","value":"a"},{"kind":"Text","value":"b"}],"saved":{"kind":"Number","value":0.0}}"#;
        let v: Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            v,
            Value::Op {
                op: Op::Join,
                args: vec![Value::Text { value: "a".into() }, Value::Text { value: "b".into() }],
                saved: Box::new(Value::number(0.0)),
            }
        );
    }

    #[test]
    fn eval_number_nested_expression() {
        // (2 + 3) * 4 == 20
        let v = Value::Op {
            op: Op::Mul,
            args: vec![
                Value::Op { op: Op::Add, args: vec![Value::number(2.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) },
                Value::number(4.0),
            ],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_number(), Ok(20.0));
    }

    #[test]
    fn eval_number_division_by_zero_errs() {
        let v = Value::Op { op: Op::Div, args: vec![Value::number(1.0), Value::number(0.0)], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn eval_number_random_integer_stays_in_range_when_both_bounds_whole() {
        let v = Value::Op { op: Op::Random, args: vec![Value::number(1.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) };
        for _ in 0..100 {
            let n = v.eval_number().unwrap();
            assert_eq!(n.fract(), 0.0);
            assert!((1.0..=3.0).contains(&n));
        }
    }

    #[test]
    fn eval_number_random_float_when_a_bound_has_a_fraction() {
        let v = Value::Op { op: Op::Random, args: vec![Value::number(1.0), Value::number(2.5)], saved: Box::new(Value::number(0.0)) };
        for _ in 0..100 {
            let n = v.eval_number().unwrap();
            assert!((1.0..=2.5).contains(&n));
        }
    }

    #[test]
    fn eval_number_random_handles_reversed_bounds() {
        let v = Value::Op { op: Op::Random, args: vec![Value::number(5.0), Value::number(1.0)], saved: Box::new(Value::number(0.0)) };
        for _ in 0..100 {
            let n = v.eval_number().unwrap();
            assert!((1.0..=5.0).contains(&n));
        }
    }

    #[test]
    fn eval_number_coerces_numeric_text() {
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::Text { value: "5".into() }, Value::number(3.0)],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_number(), Ok(8.0));
    }

    #[test]
    fn eval_number_non_numeric_text_errs() {
        let v = Value::Text { value: "abc".into() };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn get_mut_walks_path() {
        let mut v = Value::Op {
            op: Op::Add,
            args: vec![
                Value::number(1.0),
                Value::Op { op: Op::Mul, args: vec![Value::number(2.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) },
            ],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.get_mut(&[1, 0]), Some(&mut Value::number(2.0)));
        assert_eq!(v.get_mut(&[0]), Some(&mut Value::number(1.0)));
        assert_eq!(v.get_mut(&[5]), None);
    }

    #[test]
    fn eval_text_joins_two_args() {
        let v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "foo".into() }, Value::Text { value: "bar".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_text(), Ok("foobar".to_string()));
    }

    #[test]
    fn eval_text_joins_three_args_and_stringifies_numbers() {
        let v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "a".into() }, Value::number(2.0), Value::Text { value: "c".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_text(), Ok("a2c".to_string()));
    }

    #[test]
    fn get_mut_walks_join_args() {
        let mut v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "a".into() }, Value::Text { value: "b".into() }, Value::Text { value: "c".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.get_mut(&[2]), Some(&mut Value::Text { value: "c".into() }));
        assert_eq!(v.get_mut(&[3]), None);
    }

    #[test]
    fn join_tagged_shape_round_trips() {
        let v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "a".into() }, Value::Text { value: "b".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }
}
