use serde::{Deserialize, Deserializer, Serialize};

/// Arithmetic operator for a [`Value::BinaryOp`] block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

/// A small recursive expression tree backing a numeric instruction field —
/// either a number, a piece of text, or an operator applied to two nested
/// `Value`s (e.g. `(5) + (3)`). Groundwork for a future macro-wide variable
/// system: today only `Instruction::Wait` and the `MoveMouse`/`Scroll`
/// tokens embed one, always evaluated back down to a number via
/// [`Value::eval_number`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub(crate) enum Value {
    Number { value: f64 },
    Text { value: String },
    BinaryOp { op: Op, lhs: Box<Value>, rhs: Box<Value> },
}

/// The result of evaluating a [`Value`] tree — still either a number or
/// text, since a leaf can be either; only [`Value::BinaryOp`] forces its
/// operands down to a number (via [`Evaluated::as_number`]).
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

impl Value {
    pub(crate) fn number(value: f64) -> Self {
        Value::Number { value }
    }

    pub(crate) fn eval(&self) -> Result<Evaluated, String> {
        match self {
            Value::Number { value } => Ok(Evaluated::Number(*value)),
            Value::Text { value } => Ok(Evaluated::Text(value.clone())),
            Value::BinaryOp { op, lhs, rhs } => {
                let l = lhs.eval()?.as_number()?;
                let r = rhs.eval()?.as_number()?;
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

    /// Walks `path` (`0` steps into `lhs`, `1` into `rhs` at each
    /// `BinaryOp`) down to the addressed node; an empty path returns `self`.
    pub(crate) fn get_mut(&mut self, path: &[u8]) -> Option<&mut Value> {
        match path.split_first() {
            None => Some(self),
            Some((0, rest)) => match self {
                Value::BinaryOp { lhs, .. } => lhs.get_mut(rest),
                _ => None,
            },
            Some((1, rest)) => match self {
                Value::BinaryOp { rhs, .. } => rhs.get_mut(rest),
                _ => None,
            },
            Some(_) => None,
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
            Value::BinaryOp { op, lhs, rhs } => {
                2u8.hash(state);
                op.hash(state);
                lhs.hash(state);
                rhs.hash(state);
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
        #[derive(Deserialize)]
        #[serde(tag = "kind")]
        enum Tagged {
            Number { value: f64 },
            Text { value: String },
            BinaryOp { op: Op, lhs: Box<Value>, rhs: Box<Value> },
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ValueDe {
            Legacy(f64),
            Current(Tagged),
        }

        Ok(match ValueDe::deserialize(deserializer)? {
            ValueDe::Legacy(n) => Value::Number { value: n },
            ValueDe::Current(Tagged::Number { value }) => Value::Number { value },
            ValueDe::Current(Tagged::Text { value }) => Value::Text { value },
            ValueDe::Current(Tagged::BinaryOp { op, lhs, rhs }) => Value::BinaryOp { op, lhs, rhs },
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
    fn tagged_shape_round_trips() {
        let v = Value::BinaryOp {
            op: Op::Add,
            lhs: Box::new(Value::number(2.0)),
            rhs: Box::new(Value::Text { value: "3".into() }),
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn eval_number_nested_expression() {
        // (2 + 3) * 4 == 20
        let v = Value::BinaryOp {
            op: Op::Mul,
            lhs: Box::new(Value::BinaryOp {
                op: Op::Add,
                lhs: Box::new(Value::number(2.0)),
                rhs: Box::new(Value::number(3.0)),
            }),
            rhs: Box::new(Value::number(4.0)),
        };
        assert_eq!(v.eval_number(), Ok(20.0));
    }

    #[test]
    fn eval_number_division_by_zero_errs() {
        let v = Value::BinaryOp { op: Op::Div, lhs: Box::new(Value::number(1.0)), rhs: Box::new(Value::number(0.0)) };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn eval_number_coerces_numeric_text() {
        let v = Value::BinaryOp {
            op: Op::Add,
            lhs: Box::new(Value::Text { value: "5".into() }),
            rhs: Box::new(Value::number(3.0)),
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
        let mut v = Value::BinaryOp {
            op: Op::Add,
            lhs: Box::new(Value::number(1.0)),
            rhs: Box::new(Value::BinaryOp { op: Op::Mul, lhs: Box::new(Value::number(2.0)), rhs: Box::new(Value::number(3.0)) }),
        };
        assert_eq!(v.get_mut(&[1, 0]), Some(&mut Value::number(2.0)));
        assert_eq!(v.get_mut(&[0]), Some(&mut Value::number(1.0)));
        assert_eq!(v.get_mut(&[5]), None);
    }
}
