use rand::RngExt;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Operator for a [`Value::Op`] block — arithmetic, [`Op::Random`],
/// [`Op::Join`] (text concatenation), a zero-arity text constant
/// (`Op::NewLine`, `Op::Tab`), or one of the text/lookup operators below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    /// `args[0] % args[1]`, always in `[0, args[1])` (via `f64::rem_euclid`)
    /// rather than Rust's `%`, so a negative left-hand side still reads as a
    /// normal "wrap around" mod. Errs like `Div` on a zero right-hand side.
    Mod,
    /// `args[0]` rounded to the nearest whole number (half away from zero).
    Round,
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
    /// Zero-arity text constants — `args` is always empty. `eval` ignores
    /// `args` entirely for these.
    NewLine,
    Tab,
    /// 1-based index of `args[0]` (the needle) inside `args[1]` (the
    /// haystack), both as text; `0` if not found. Character-based, not
    /// byte-based, so it agrees with `Op::LetterOf`'s indexing.
    IndexOf,
    /// Same as `IndexOf`, but the index of the last occurrence.
    LastIndexOf,
    /// The single character at `args[0]` (1-based) of `args[1]` (as text).
    /// Errs if the index is out of range.
    LetterOf,
    /// Character count of `args[0]` (as text).
    Length,
    /// Upper/lowercases `args[0]` (as text) depending on `args[1]`, which is
    /// never a user-editable value slot despite being a plain `Value::Text`
    /// leaf like any other arg — it's driven entirely by an in-place
    /// dropdown (`"Upper"`/`"Lower"`) rather than the drag/drop machinery,
    /// same spirit as `Random`'s bounds not being "operands". See
    /// `OPERATOR_KINDS`'s `"Case"` row and the frontend's `enumArg` spec.
    Case,
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
pub enum Value {
    Number { value: f64 },
    Text { value: String },
    Op { op: Op, args: Vec<Value>, saved: Box<Value> },
    /// A read of a macro-wide variable by name — a leaf like `Number`/`Text`
    /// (no args, no `saved` slot), resolved against the running macro's
    /// variable store by [`Value::resolve_vars`] before `eval` ever sees it.
    Var { name: String },
    /// A read of the current custom-block invocation's bound parameter by
    /// name — a leaf like `Var`, but resolved against that invocation's
    /// local parameter scope (`macros::runner::ExecCtx::param_env`) rather
    /// than the macro-wide variable store, since two concurrent/nested
    /// invocations of the same block must not share one slot. Only ever
    /// produced by dragging one of a block's own declared inputs (rendered
    /// on its header strand) — see `BlockDef`.
    Param { name: String },
    /// Value-position invocation of a `returns_value == true` custom block
    /// — mirrors `Op`'s shape (including `saved`, so swapping it in/out of a
    /// slot round-trips like any other operator) but names a custom block
    /// instead of a fixed [`Op`]. Resolved by
    /// `macros::runner::resolve_calls_and_params` before `eval` ever sees
    /// it, exactly like `Var`/`Param`.
    Call { block_id: String, args: Vec<Value>, saved: Box<Value> },
}

/// The result of evaluating a [`Value`] tree — still either a number or
/// text, since a leaf can be either; only an arithmetic/`Random` [`Op`]
/// forces its args down to a number (via [`Evaluated::as_number`]). Also
/// doubles as the persisted representation of a variable's current value
/// (see `macros::VariableDef`), so it derives the traits that needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Evaluated {
    Number(f64),
    Text(String),
}

/// Manual impl mirroring `Value`'s own — `f64` isn't `Hash`, so its bit
/// pattern stands in.
impl std::hash::Hash for Evaluated {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Evaluated::Number(n) => {
                0u8.hash(state);
                n.to_bits().hash(state);
            }
            Evaluated::Text(s) => {
                1u8.hash(state);
                s.hash(state);
            }
        }
    }
}

impl Evaluated {
    /// Coerces the evaluated result to a number, parsing text loosely (so a
    /// `Text` leaf holding `"5"` still works as an operand); fails with a
    /// readable message for non-numeric text.
    pub fn as_number(&self) -> Result<f64, String> {
        match self {
            Evaluated::Number(n) => Ok(*n),
            Evaluated::Text(s) => s.trim().parse::<f64>().map_err(|_| format!("\"{s}\" is not a number")),
        }
    }
}

/// Static per-palette-entry metadata driving default construction when a
/// block is dropped from the sidebar — the single place a new operator's
/// arity and default arguments need registering. Arity lives on the *kind
/// string*, not on `Op` itself, since `Op::Join` alone doesn't have one true
/// arity (2 vs 3 is just which palette entry it came from).
/// `commands.rs`'s `apply_value_kind` is the only consumer.
pub struct OperatorKindSpec {
    pub kind: &'static str,
    pub op: Op,
    pub arity: usize,
    /// Builds the full default `args` vec (one entry per arg, in order) —
    /// a function rather than a fixed `Vec` since `Value` isn't `Const`.
    /// Per-index rather than a single repeated default so mixed-type
    /// operators (e.g. `LetterOf`'s number-then-text pair) can give each
    /// slot its own default.
    pub default_args: fn() -> Vec<Value>,
}

fn text_default() -> Value {
    Value::Text { value: String::new() }
}

pub const OPERATOR_KINDS: &[OperatorKindSpec] = &[
    OperatorKindSpec { kind: "Add", op: Op::Add, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Sub", op: Op::Sub, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Mul", op: Op::Mul, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Div", op: Op::Div, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Mod", op: Op::Mod, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Round", op: Op::Round, arity: 1, default_args: || vec![Value::number(0.0)] },
    OperatorKindSpec { kind: "Random", op: Op::Random, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Join", op: Op::Join, arity: 2, default_args: || vec![text_default(), text_default()] },
    OperatorKindSpec { kind: "Join3", op: Op::Join, arity: 3, default_args: || vec![text_default(), text_default(), text_default()] },
    // `default_args` is unused for these — arity 0 means it's never called.
    OperatorKindSpec { kind: "NewLine", op: Op::NewLine, arity: 0, default_args: Vec::new },
    OperatorKindSpec { kind: "Tab", op: Op::Tab, arity: 0, default_args: Vec::new },
    OperatorKindSpec { kind: "IndexOf", op: Op::IndexOf, arity: 2, default_args: || vec![text_default(), text_default()] },
    OperatorKindSpec { kind: "LastIndexOf", op: Op::LastIndexOf, arity: 2, default_args: || vec![text_default(), text_default()] },
    OperatorKindSpec { kind: "LetterOf", op: Op::LetterOf, arity: 2, default_args: || vec![Value::number(1.0), text_default()] },
    OperatorKindSpec { kind: "Length", op: Op::Length, arity: 1, default_args: || vec![text_default()] },
    // `args[1]` defaults to the dropdown's "uppercase" option — see
    // `Op::Case`'s doc comment.
    OperatorKindSpec {
        kind: "Case",
        op: Op::Case,
        arity: 2,
        default_args: || vec![text_default(), Value::Text { value: "Upper".to_string() }],
    },
];

/// 1-based char index of the first occurrence of `needle` in `haystack`, or
/// `0` if `needle` is empty, longer than `haystack`, or not found. Scans by
/// `char`, not byte, so it agrees with `Op::LetterOf`'s indexing on
/// multi-byte text.
fn char_index_of(haystack: &str, needle: &str) -> usize {
    let h: Vec<char> = haystack.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || n.len() > h.len() {
        return 0;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()] == n[..]).map_or(0, |i| i + 1)
}

/// Same as [`char_index_of`], but the last occurrence.
fn char_last_index_of(haystack: &str, needle: &str) -> usize {
    let h: Vec<char> = haystack.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || n.len() > h.len() {
        return 0;
    }
    (0..=h.len() - n.len()).rev().find(|&i| h[i..i + n.len()] == n[..]).map_or(0, |i| i + 1)
}

impl Value {
    pub fn number(value: f64) -> Self {
        Value::Number { value }
    }

    pub fn eval(&self) -> Result<Evaluated, String> {
        match self {
            Value::Number { value } => Ok(Evaluated::Number(*value)),
            Value::Text { value } => Ok(Evaluated::Text(value.clone())),
            // Every real call site resolves vars via `resolve_vars` first —
            // this only fires if that step was skipped, so it reads as a
            // bug rather than something a user's macro can trigger.
            Value::Var { .. } => Err("unresolved variable reference".to_string()),
            // Same invariant as `Var` above, just for the two node kinds
            // only `macros::runner::resolve_calls_and_params` can resolve
            // (it needs to actually run instructions to resolve a `Call`,
            // which this module deliberately has no access to) — this is
            // also what keeps `preview_value`/`apply_value_kind`'s
            // best-effort collapse automatically safe: they only ever call
            // `resolve_vars(...).eval()`, never the runner's resolution
            // pass, so a value containing a block call just surfaces as an
            // ordinary eval error there instead of running anything.
            Value::Param { .. } => Err("unresolved parameter reference".to_string()),
            Value::Call { .. } => Err("custom block calls can't be evaluated directly".to_string()),
            Value::Op { op: Op::Join, args, .. } => {
                let mut s = String::new();
                for a in args {
                    s.push_str(&a.eval_text()?);
                }
                Ok(Evaluated::Text(s))
            }
            Value::Op { op: Op::NewLine, .. } => Ok(Evaluated::Text("\n".to_string())),
            Value::Op { op: Op::Tab, .. } => Ok(Evaluated::Text("\t".to_string())),
            Value::Op { op: Op::Length, args, .. } => Ok(Evaluated::Number(args[0].eval_text()?.chars().count() as f64)),
            Value::Op { op: Op::IndexOf, args, .. } => {
                let needle = args[0].eval_text()?;
                let haystack = args[1].eval_text()?;
                Ok(Evaluated::Number(char_index_of(&haystack, &needle) as f64))
            }
            Value::Op { op: Op::LastIndexOf, args, .. } => {
                let needle = args[0].eval_text()?;
                let haystack = args[1].eval_text()?;
                Ok(Evaluated::Number(char_last_index_of(&haystack, &needle) as f64))
            }
            Value::Op { op: Op::LetterOf, args, .. } => {
                let index = args[0].eval_number()? as i64;
                let text = args[1].eval_text()?;
                let chars: Vec<char> = text.chars().collect();
                if index < 1 || index as usize > chars.len() {
                    return Err(format!("letter {index} is out of range for a {}-character value", chars.len()));
                }
                Ok(Evaluated::Text(chars[index as usize - 1].to_string()))
            }
            Value::Op { op: Op::Case, args, .. } => {
                let text = args[0].eval_text()?;
                let upper = args[1].eval_text()? == "Upper";
                Ok(Evaluated::Text(if upper { text.to_uppercase() } else { text.to_lowercase() }))
            }
            Value::Op { op: Op::Round, args, .. } => Ok(Evaluated::Number(args[0].eval_number()?.round())),
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
                    Op::Mod => {
                        if r == 0.0 {
                            return Err("mod by zero".to_string());
                        }
                        l.rem_euclid(r)
                    }
                    Op::Random => {
                        let (lo, hi) = (l.min(r), l.max(r));
                        if l.fract() == 0.0 && r.fract() == 0.0 {
                            rand::rng().random_range(lo as i64..=hi as i64) as f64
                        } else {
                            rand::rng().random_range(lo..=hi)
                        }
                    }
                    Op::Join | Op::NewLine | Op::Tab | Op::Length | Op::IndexOf | Op::LastIndexOf | Op::LetterOf | Op::Case
                    | Op::Round => unreachable!("matched above"),
                };
                Ok(Evaluated::Number(result))
            }
        }
    }

    /// Evaluates the tree and coerces the result to a number — the entry
    /// point every numeric instruction field actually calls.
    pub fn eval_number(&self) -> Result<f64, String> {
        self.eval()?.as_number()
    }

    /// Evaluates the tree down to a string — the entry point the `Text`
    /// instruction token calls. A `Text` leaf or a text-producing `Op`
    /// (`Join`, `NewLine`, `Tab`, `LetterOf`, `Case`) result passes through
    /// verbatim; a `Number` leaf or another `Op` result (always numeric,
    /// even for `Op::Random`) gets stringified.
    pub fn eval_text(&self) -> Result<String, String> {
        Ok(match self.eval()? {
            Evaluated::Text(s) => s,
            Evaluated::Number(n) => n.to_string(),
        })
    }

    /// Walks `path` (`n` steps into `args[n]` at each `Op`) down to the
    /// addressed node; an empty path returns `self`.
    pub fn get_mut(&mut self, path: &[u8]) -> Option<&mut Value> {
        match path.split_first() {
            None => Some(self),
            Some((&step, rest)) => match self {
                Value::Op { args, .. } | Value::Call { args, .. } => args.get_mut(step as usize)?.get_mut(rest),
                _ => None,
            },
        }
    }

    /// Renames every [`Value::Var`] leaf reading `old` (anywhere in this
    /// tree, including `saved`) to `new` — used by `Macro::rename_variable`
    /// so an in-progress macro's existing reporter/setter/getter blocks keep
    /// working after a variable is renamed, rather than being silently
    /// orphaned.
    pub fn rename_var(&mut self, old: &str, new: &str) {
        match self {
            Value::Var { name } => {
                if name == old {
                    *name = new.to_string();
                }
            }
            Value::Op { args, saved, .. } | Value::Call { args, saved, .. } => {
                for arg in args.iter_mut() {
                    arg.rename_var(old, new);
                }
                saved.rename_var(old, new);
            }
            Value::Number { .. } | Value::Text { .. } | Value::Param { .. } => {}
        }
    }

    /// Renames every [`Value::Param`] leaf reading `old` (anywhere in this
    /// tree, including `saved`) to `new` — counterpart to `rename_var`, used
    /// by `macros::Macro::rename_block_input` so a custom block's own body
    /// keeps working after one of its inputs is renamed.
    pub fn rename_param(&mut self, old: &str, new: &str) {
        match self {
            Value::Param { name } => {
                if name == old {
                    *name = new.to_string();
                }
            }
            Value::Op { args, saved, .. } | Value::Call { args, saved, .. } => {
                for arg in args.iter_mut() {
                    arg.rename_param(old, new);
                }
                saved.rename_param(old, new);
            }
            Value::Number { .. } | Value::Text { .. } | Value::Var { .. } => {}
        }
    }

    /// Applies `f` to the `args` of every [`Value::Call`] node (anywhere in
    /// this tree, including nested inside another call's own `args`) that
    /// references `block_id` — used by `macros::Macro::insert_block_input`/
    /// `remove_block_input` to keep every call site's argument list
    /// positionally aligned with the block's current `pieces`.
    pub fn for_each_call_args_mut(&mut self, block_id: &str, f: &mut dyn FnMut(&mut Vec<Value>)) {
        match self {
            Value::Number { .. } | Value::Text { .. } | Value::Var { .. } | Value::Param { .. } => {}
            Value::Op { args, saved, .. } => {
                for a in args.iter_mut() {
                    a.for_each_call_args_mut(block_id, f);
                }
                saved.for_each_call_args_mut(block_id, f);
            }
            Value::Call { block_id: id, args, saved } => {
                if id == block_id {
                    f(args);
                }
                for a in args.iter_mut() {
                    a.for_each_call_args_mut(block_id, f);
                }
                saved.for_each_call_args_mut(block_id, f);
            }
        }
    }

    /// Replaces every [`Value::Call`] node (anywhere in this tree)
    /// referencing `block_id` with a plain `0` leaf — used by
    /// `macros::Macro::remove_block` so deleting a custom block scrubs every
    /// value-position reference instead of leaving a dangling one.
    pub fn scrub_block_calls(&mut self, block_id: &str) {
        match self {
            Value::Number { .. } | Value::Text { .. } | Value::Var { .. } | Value::Param { .. } => {}
            Value::Op { args, saved, .. } => {
                for a in args.iter_mut() {
                    a.scrub_block_calls(block_id);
                }
                saved.scrub_block_calls(block_id);
            }
            Value::Call { block_id: id, args, saved } => {
                if id == block_id {
                    *self = Value::number(0.0);
                } else {
                    for a in args.iter_mut() {
                        a.scrub_block_calls(block_id);
                    }
                    saved.scrub_block_calls(block_id);
                }
            }
        }
    }

    /// Rebuilds this tree with every [`Value::Var`] leaf replaced by a
    /// `Number`/`Text` leaf holding its current value from `env` (or `0` if
    /// the name isn't in `env` — shouldn't happen in practice, but a stray
    /// reference must still evaluate to something rather than erroring the
    /// whole tree). Must be called before `eval`/`eval_number`/`eval_text`
    /// on any tree that might contain a variable read.
    pub fn resolve_vars(&self, env: &HashMap<String, Evaluated>) -> Value {
        match self {
            Value::Number { value } => Value::Number { value: *value },
            Value::Text { value } => Value::Text { value: value.clone() },
            Value::Op { op, args, saved } => Value::Op {
                op: *op,
                args: args.iter().map(|a| a.resolve_vars(env)).collect(),
                saved: Box::new(saved.resolve_vars(env)),
            },
            Value::Var { name } => match env.get(name) {
                Some(Evaluated::Number(n)) => Value::Number { value: *n },
                Some(Evaluated::Text(s)) => Value::Text { value: s.clone() },
                None => Value::number(0.0),
            },
            // Left untouched here — `Param` is resolved against the current
            // invocation's parameter scope, and `Call` needs to actually run
            // the callee's body; both require execution capability this
            // module deliberately doesn't have. Only their nested `args`
            // (which may themselves contain `Var` reads) are recursed into.
            // See `macros::runner::resolve_calls_and_params`, which always
            // runs on the output of this method.
            Value::Param { name } => Value::Param { name: name.clone() },
            Value::Call { block_id, args, saved } => Value::Call {
                block_id: block_id.clone(),
                args: args.iter().map(|a| a.resolve_vars(env)).collect(),
                saved: Box::new(saved.resolve_vars(env)),
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
            Value::Var { name } => {
                3u8.hash(state);
                name.hash(state);
            }
            Value::Param { name } => {
                4u8.hash(state);
                name.hash(state);
            }
            Value::Call { block_id, args, saved } => {
                5u8.hash(state);
                block_id.hash(state);
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
            Var { name: String },
            Param { name: String },
            Call {
                block_id: String,
                args: Vec<Value>,
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
            ValueDe::Current(Tagged::Var { name }) => Value::Var { name },
            ValueDe::Current(Tagged::Param { name }) => Value::Param { name },
            ValueDe::Current(Tagged::Call { block_id, args, saved }) => Value::Call { block_id, args, saved },
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
    fn eval_text_new_line_is_constant_regardless_of_args() {
        let v = Value::Op { op: Op::NewLine, args: vec![], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("\n".to_string()));
    }

    #[test]
    fn eval_text_tab_is_constant_regardless_of_args() {
        let v = Value::Op { op: Op::Tab, args: vec![], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("\t".to_string()));
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

    fn text(s: &str) -> Value {
        Value::Text { value: s.to_string() }
    }

    #[test]
    fn eval_number_mod_wraps_like_rem_euclid() {
        let v = Value::Op { op: Op::Mod, args: vec![Value::number(-1.0), Value::number(5.0)], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(4.0));
    }

    #[test]
    fn eval_number_mod_by_zero_errs() {
        let v = Value::Op { op: Op::Mod, args: vec![Value::number(1.0), Value::number(0.0)], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn eval_number_round_rounds_half_away_from_zero() {
        let v = Value::Op { op: Op::Round, args: vec![Value::number(2.5)], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(3.0));
    }

    #[test]
    fn eval_number_length_counts_chars_not_bytes() {
        let v = Value::Op { op: Op::Length, args: vec![text("héllo")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(5.0));
    }

    #[test]
    fn eval_number_index_of_is_one_based() {
        let v = Value::Op { op: Op::IndexOf, args: vec![text("lo"), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(4.0));
    }

    #[test]
    fn eval_number_index_of_not_found_is_zero() {
        let v = Value::Op { op: Op::IndexOf, args: vec![text("xyz"), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(0.0));
    }

    #[test]
    fn eval_number_last_index_of_finds_final_occurrence() {
        let v = Value::Op { op: Op::LastIndexOf, args: vec![text("l"), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(4.0));
    }

    #[test]
    fn eval_text_letter_of_is_one_based() {
        let v = Value::Op { op: Op::LetterOf, args: vec![Value::number(1.0), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("h".to_string()));
    }

    #[test]
    fn eval_text_letter_of_out_of_range_errs() {
        let v = Value::Op { op: Op::LetterOf, args: vec![Value::number(6.0), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_text().is_err());
    }

    #[test]
    fn eval_text_letter_of_rejects_zero_index() {
        let v = Value::Op { op: Op::LetterOf, args: vec![Value::number(0.0), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_text().is_err());
    }

    #[test]
    fn eval_text_case_uppercases_by_default_flag() {
        let v = Value::Op { op: Op::Case, args: vec![text("Hello"), text("Upper")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("HELLO".to_string()));
    }

    #[test]
    fn eval_text_case_lowercases_when_flagged() {
        let v = Value::Op { op: Op::Case, args: vec![text("Hello"), text("Lower")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("hello".to_string()));
    }

    #[test]
    fn resolve_vars_replaces_leaf_with_current_number_value() {
        let env = HashMap::from([("x".to_string(), Evaluated::Number(5.0))]);
        let v = Value::Var { name: "x".to_string() };
        assert_eq!(v.resolve_vars(&env), Value::number(5.0));
    }

    #[test]
    fn resolve_vars_replaces_leaf_with_current_text_value() {
        let env = HashMap::from([("s".to_string(), Evaluated::Text("hi".to_string()))]);
        let v = Value::Var { name: "s".to_string() };
        assert_eq!(v.resolve_vars(&env), text("hi"));
    }

    #[test]
    fn resolve_vars_defaults_missing_name_to_zero() {
        let v = Value::Var { name: "missing".to_string() };
        assert_eq!(v.resolve_vars(&HashMap::new()), Value::number(0.0));
    }

    #[test]
    fn resolve_vars_recurses_into_op_args_and_saved() {
        let env = HashMap::from([("x".to_string(), Evaluated::Number(2.0))]);
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::Var { name: "x".to_string() }, Value::number(3.0)],
            saved: Box::new(Value::Var { name: "x".to_string() }),
        };
        let resolved = v.resolve_vars(&env);
        assert_eq!(resolved.eval_number(), Ok(5.0));
        match resolved {
            Value::Op { saved, .. } => assert_eq!(*saved, Value::number(2.0)),
            _ => panic!("expected Op"),
        }
    }

    #[test]
    fn eval_errs_on_unresolved_var() {
        let v = Value::Var { name: "x".to_string() };
        assert!(v.eval().is_err());
    }

    #[test]
    fn rename_var_renames_matching_leaf() {
        let mut v = Value::Var { name: "x".to_string() };
        v.rename_var("x", "y");
        assert_eq!(v, Value::Var { name: "y".to_string() });
    }

    #[test]
    fn rename_var_ignores_non_matching_leaf() {
        let mut v = Value::Var { name: "x".to_string() };
        v.rename_var("z", "y");
        assert_eq!(v, Value::Var { name: "x".to_string() });
    }

    #[test]
    fn rename_var_recurses_into_op_args_and_saved() {
        let mut v = Value::Op {
            op: Op::Add,
            args: vec![Value::Var { name: "x".to_string() }, Value::number(3.0)],
            saved: Box::new(Value::Var { name: "x".to_string() }),
        };
        v.rename_var("x", "y");
        match v {
            Value::Op { args, saved, .. } => {
                assert_eq!(args[0], Value::Var { name: "y".to_string() });
                assert_eq!(*saved, Value::Var { name: "y".to_string() });
            }
            _ => panic!("expected Op"),
        }
    }
}
