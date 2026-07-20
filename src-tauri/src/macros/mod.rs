use crate::input::types::InputToken;
use crate::input::value::{Op, Value};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) mod backend;
pub(crate) mod priority;
pub(crate) mod runner;
pub(crate) mod thread_pool;
pub(crate) mod thread;
pub(crate) mod loop_control;

fn default_macro_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_strand_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_speed_multiplier() -> f64 {
    1.0
}

/// Id used by strands' "main" entry point before "When Ran" blocks existed —
/// every macro had exactly one strand with this literal id, and it was the
/// only thing ever executed. Kept around solely so loading an old save file
/// can find that strand and migrate it (see `From<MacroDe>` below); it has no
/// special meaning anywhere else anymore.
const LEGACY_ROOT_STRAND_ID: &str = "root";

/// One draggable stack of instructions on the canvas. A strand is an entry
/// point — one of the possibly-many independent things a macro runs
/// concurrently — when its first instruction is `Instruction::WhenRan`;
/// every other strand is still persisted with the macro (so stray/detached
/// stacks survive a save/reload) but stays inert until dragged under a
/// "When Ran" block.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) struct Strand {
    #[serde(default = "default_strand_id")]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) x: i32,
    #[serde(default)]
    pub(crate) y: i32,
    #[serde(default)]
    pub(crate) instructions: Vec<Instruction>,
}

/// A value block sitting free on the canvas, not embedded in any
/// instruction's field — the drag-and-drop "parking spot" for a value block
/// before/after it's placed into a field's slot (or one of its subfields).
/// Can never be attached to a strand's instruction list directly.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) struct FloatingValue {
    #[serde(default = "default_floating_value_id")]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) x: i32,
    #[serde(default)]
    pub(crate) y: i32,
    pub(crate) value: Value,
}

fn default_floating_value_id() -> String {
    Uuid::new_v4().simple().to_string()
}

impl Instruction {
    /// Returns `true` for "header" blocks — blocks that must be first in their
    /// strand, cannot have anything stacked above them, and render with a flat
    /// top edge (no connector notch). Currently only `WhenRan`.
    pub(crate) fn is_header(&self) -> bool {
        matches!(self, Instruction::WhenRan)
    }
}

impl Strand {
    pub(crate) fn starts_with_when_ran(&self) -> bool {
        self.instructions.first().map_or(false, Instruction::is_header)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "MacroDe")]
pub(crate) struct Macro {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) strands: Vec<Strand>,
    /// Strand explicitly chosen (via the canvas's "Set Recording Target"
    /// right-click action) to receive freshly-recorded input. `None` means
    /// no explicit choice has been made yet — falls back to the automatic
    /// "first When Ran strand, else first strand" rule. Deliberately kept
    /// out of the undo/redo stacks (which only snapshot `strands`): it's a
    /// pointer/preference, not an edit to the macro's instructions.
    #[serde(default)]
    pub(crate) recording_target: Option<String>,
    /// Playback speed for this macro: every `Wait` instruction's duration is
    /// divided by this when the macro runs, combined with the
    /// global runtime override (see `AppState::global_speed_multiplier`).
    /// 1.0 is normal speed, 2.0 is twice as fast (waits are half as long),
    /// 0.5 is half as fast (waits are twice as long). Clamped to
    /// `SPEED_MULTIPLIER_RANGE` wherever it's set.
    #[serde(default = "default_speed_multiplier")]
    pub(crate) speed_multiplier: f64,
    /// Value blocks parked on open canvas — see `FloatingValue`.
    #[serde(default)]
    pub(crate) floating_values: Vec<FloatingValue>,
}

/// Valid range for both the per-macro and global speed multipliers, enforced
/// wherever either is set from user input.
pub(crate) const SPEED_MULTIPLIER_RANGE: std::ops::RangeInclusive<f64> = 0.1..=10.0;

/// Deserialization shape supporting both the current multi-strand format and
/// the legacy single flat `code: Vec<Instruction>` format saved by older
/// versions of the app. Whichever variant matches the JSON on disk wins;
/// legacy macros become a single root strand holding their old instructions.
#[derive(Deserialize)]
#[serde(untagged)]
enum MacroDe {
    Current {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        strands: Vec<Strand>,
        #[serde(default)]
        recording_target: Option<String>,
        #[serde(default = "default_speed_multiplier")]
        speed_multiplier: f64,
        #[serde(default)]
        floating_values: Vec<FloatingValue>,
    },
    Legacy {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        code: Vec<Instruction>,
    },
}

impl From<MacroDe> for Macro {
    fn from(de: MacroDe) -> Self {
        match de {
            MacroDe::Current { id, name, description, mut strands, recording_target, speed_multiplier, floating_values } => {
                // Pre-"When Ran" saves have a strand literally id=="root" that
                // was the sole implicit entry point; give it a real WhenRan
                // block so it keeps running after upgrade.
                if let Some(legacy) = strands.iter_mut().find(|s| s.id == LEGACY_ROOT_STRAND_ID) {
                    if !legacy.starts_with_when_ran() {
                        legacy.instructions.insert(0, Instruction::WhenRan);
                    }
                }
                Self { id, name, description, strands, recording_target, speed_multiplier, floating_values }
            }
            MacroDe::Legacy { id, name, description, mut code } => {
                code.insert(0, Instruction::WhenRan);
                let strand = Strand { id: default_strand_id(), x: 0, y: 0, instructions: code };
                Self { id, name, description, strands: vec![strand], recording_target: None, speed_multiplier: default_speed_multiplier(), floating_values: Vec::new() }
            }
        }
    }
}

impl Macro {
    pub(crate) fn new(name: String, description: String, mut code: Vec<Instruction>) -> Self {
        code.insert(0, Instruction::WhenRan);
        let strand = Strand { id: default_strand_id(), x: 0, y: 0, instructions: code };
        Self {
            id: default_macro_id(),
            name,
            description,
            strands: vec![strand],
            recording_target: None,
            speed_multiplier: default_speed_multiplier(),
            floating_values: Vec::new(),
        }
    }

    pub(crate) fn ensure_id(&mut self) {
        if self.id.trim().is_empty() {
            self.id = default_macro_id();
        }
    }

    pub(crate) fn strand(&self, id: &str) -> Option<&Strand> {
        self.strands.iter().find(|s| s.id == id)
    }

    pub(crate) fn strand_mut(&mut self, id: &str) -> Option<&mut Strand> {
        self.strands.iter_mut().find(|s| s.id == id)
    }

    pub(crate) fn floating_value_mut(&mut self, id: &str) -> Option<&mut FloatingValue> {
        self.floating_values.iter_mut().find(|f| f.id == id)
    }

    /// Strand that freshly-recorded input gets appended to: the explicitly
    /// chosen `recording_target` if it still exists, else the first "When
    /// Ran" entry point if one exists, otherwise just the first strand
    /// (creating one if the macro is completely empty) so recorded input
    /// always lands somewhere.
    pub(crate) fn recording_target_mut(&mut self) -> &mut Strand {
        if let Some(id) = &self.recording_target {
            if let Some(pos) = self.strands.iter().position(|s| &s.id == id) {
                return &mut self.strands[pos];
            }
        }
        if let Some(pos) = self.strands.iter().position(Strand::starts_with_when_ran) {
            return &mut self.strands[pos];
        }
        if self.strands.is_empty() {
            self.strands.push(Strand { id: default_strand_id(), x: 0, y: 0, instructions: vec![] });
        }
        &mut self.strands[0]
    }

    /// Read-only counterpart to `recording_target_mut` — same resolution
    /// order, but never creates a strand, so it's safe to call for display
    /// purposes (e.g. deciding which strand's top block gets the red
    /// recording-target dot).
    pub(crate) fn recording_target_id(&self) -> Option<String> {
        if let Some(id) = &self.recording_target {
            if self.strands.iter().any(|s| &s.id == id) {
                return Some(id.clone());
            }
        }
        if let Some(strand) = self.strands.iter().find(|s| s.starts_with_when_ran()) {
            return Some(strand.id.clone());
        }
        self.strands.first().map(|s| s.id.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "InstructionDe")]
pub(crate) enum Instruction {
    Token(InputToken),
    Wait(Value),
    Command(String),
    Comment(String),
    /// Marks a strand as an entry point: the strand containing it (always at
    /// index 0 — enforced by every command that can move/insert blocks) runs
    /// as its own concurrent thread when the macro is run. A macro can have
    /// any number of these.
    WhenRan,
}

impl std::hash::Hash for Macro {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.name.hash(state);
        self.description.hash(state);
        self.strands.hash(state);
        self.recording_target.hash(state);
        self.speed_multiplier.to_bits().hash(state);
        self.floating_values.hash(state);
    }
}

impl std::hash::Hash for Instruction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Token(t)   => { 0u8.hash(state); t.hash(state); }
            Self::Wait(d)    => { 1u8.hash(state); d.hash(state); }
            Self::Command(s) => { 2u8.hash(state); s.hash(state); }
            Self::Comment(s) => { 3u8.hash(state); s.hash(state); }
            Self::WhenRan    => { 4u8.hash(state); }
        }
    }
}

#[derive(Deserialize)]
enum InstructionDe {
    Token(InputToken),
    Wait(WaitDe),
    Command(String),
    Comment(String),
    WhenRan,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WaitDe {
    /// Oldest save shape: a bare duration, no randomness field existed yet.
    LegacyNumber(u64),
    /// Pre-`Op::Random` save shape: `[duration, randomness]` — randomness
    /// was a dedicated field rather than something expressed inside the
    /// `Value` tree. Migrated into `Op::Random` below rather than dropped,
    /// so old macros keep producing the same spread of wait times.
    LegacyWithRandomness(Value, Value),
    /// Current shape: a single `Value` (a plain duration, or a duration
    /// wrapped in any operator including `Op::Random`).
    Current(Value),
}

/// Folds a legacy `[duration, randomness]` `Wait` into today's single-`Value`
/// shape: `duration` alone if there was never any randomness configured,
/// otherwise `duration` wrapped in `Op::Random` spanning
/// `[duration - randomness, duration + randomness]` — the same uniform
/// spread the old dedicated-field jitter produced (see the removed
/// offset/sign logic this replaced in `runner.rs`), just expressed as a
/// value-tree node instead of a second instruction field.
fn migrate_wait_duration(duration: Value, randomness: Value) -> Value {
    if randomness == Value::number(0.0) {
        return duration;
    }
    let zero = || Box::new(Value::number(0.0));
    Value::Op {
        op: Op::Random,
        args: vec![
            Value::Op { op: Op::Sub, args: vec![duration.clone(), randomness.clone()], saved: zero() },
            Value::Op { op: Op::Add, args: vec![duration, randomness], saved: zero() },
        ],
        saved: zero(),
    }
}

impl From<InstructionDe> for Instruction {
    fn from(de: InstructionDe) -> Self {
        match de {
            InstructionDe::Token(t) => Instruction::Token(t),
            InstructionDe::Wait(WaitDe::LegacyNumber(d)) => Instruction::Wait(Value::number(d as f64)),
            InstructionDe::Wait(WaitDe::LegacyWithRandomness(d, r)) => Instruction::Wait(migrate_wait_duration(d, r)),
            InstructionDe::Wait(WaitDe::Current(d)) => Instruction::Wait(d),
            InstructionDe::Command(s) => Instruction::Command(s),
            InstructionDe::Comment(s) => Instruction::Comment(s),
            InstructionDe::WhenRan => Instruction::WhenRan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::types::Coordinate;

    #[test]
    fn new_macro_defaults_to_one_when_ran_strand() {
        let mac = Macro::new("Test".into(), "".into(), vec![]);
        assert_eq!(mac.strands.len(), 1);
        assert!(mac.strands[0].starts_with_when_ran());
    }

    #[test]
    fn legacy_flat_code_migrates_to_when_ran_strand() {
        let json = r#"{"name":"Old","description":"","code":[{"Comment":"hi"}]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(mac.strands.len(), 1);
        assert_eq!(mac.strands[0].instructions, vec![Instruction::WhenRan, Instruction::Comment("hi".into())]);
    }

    #[test]
    fn legacy_root_strand_gains_when_ran_on_load() {
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":[{"Comment":"hi"}]},
            {"id":"stray","x":10,"y":10,"instructions":[]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        let root = mac.strand("root").unwrap();
        assert!(root.starts_with_when_ran());
        assert_eq!(root.instructions, vec![Instruction::WhenRan, Instruction::Comment("hi".into())]);
        // Untouched, non-entry strand should survive as-is.
        assert!(mac.strand("stray").unwrap().instructions.is_empty());
    }

    #[test]
    fn already_migrated_root_strand_is_not_double_prepended() {
        let json = r#"{"id":"m1","name":"New","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",{"Comment":"hi"}]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(mac.strand("root").unwrap().instructions, vec![Instruction::WhenRan, Instruction::Comment("hi".into())]);
    }

    #[test]
    fn legacy_wait_with_randomness_migrates_to_random_op() {
        let json = r#"{"Wait":[1000.0,50.0]}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::Wait(Value::Op {
                op: Op::Random,
                args: vec![
                    Value::Op {
                        op: Op::Sub,
                        args: vec![Value::number(1000.0), Value::number(50.0)],
                        saved: Box::new(Value::number(0.0)),
                    },
                    Value::Op {
                        op: Op::Add,
                        args: vec![Value::number(1000.0), Value::number(50.0)],
                        saved: Box::new(Value::number(0.0)),
                    },
                ],
                saved: Box::new(Value::number(0.0)),
            })
        );
    }

    #[test]
    fn legacy_wait_with_zero_randomness_migrates_to_plain_duration() {
        let json = r#"{"Wait":[1000.0,0.0]}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(ins, Instruction::Wait(Value::number(1000.0)));
    }

    #[test]
    fn legacy_single_arg_wait_migrates_to_value() {
        let json = r#"{"Wait":1000}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(ins, Instruction::Wait(Value::number(1000.0)));
    }

    #[test]
    fn legacy_bare_number_move_mouse_fields_migrate_to_value() {
        let json = r#"{"Token":{"MoveMouse":[5,10,"Rel"]}}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::Token(InputToken::MoveMouse(Value::number(5.0), Value::number(10.0), Coordinate::Rel)),
        );
    }
}
