use crate::input::types::InputToken;
use crate::input::value::{Evaluated, Op, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub mod backend;
pub mod priority;
pub mod run_registry;
pub mod runner;
pub mod thread_pool;
pub mod loop_control;

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
pub struct Strand {
    #[serde(default = "default_strand_id")]
    pub id: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub instructions: Vec<Instruction>,
}

/// A value block sitting free on the canvas, not embedded in any
/// instruction's field — the drag-and-drop "parking spot" for a value block
/// before/after it's placed into a field's slot (or one of its subfields).
/// Can never be attached to a strand's instruction list directly.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct FloatingValue {
    #[serde(default = "default_floating_value_id")]
    pub id: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    pub value: Value,
}

fn default_floating_value_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_variable_value() -> Evaluated {
    Evaluated::Number(0.0)
}

/// A user-declared macro-wide variable and its current value. The name is
/// fixed at creation (no rename/delete UI yet); the value is mutated by
/// `Instruction::SetVariable`/`ChangeVariable` at runtime and persisted with
/// the macro so it survives an app restart — see `AppState::variable_values`
/// for the live store this is synced from/to.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct VariableDef {
    pub name: String,
    #[serde(default = "default_variable_value")]
    pub value: Evaluated,
}

/// One piece of a custom block's prototype, in declaration order — either
/// static label text or a named input slot. `Input`'s `name` is the
/// parameter name, read inside the block's own body via `Value::Param`, and
/// unique within that block (enforced at creation/edit time in
/// `commands.rs`, not here). Both variants' `id` is a stable, opaque
/// identifier generated once when the piece is first added (client-side,
/// see `MakeBlockDialog.vue`) and never regenerated — the only way
/// `edit_block` can tell "this input was renamed" apart from "this input
/// was removed and an unrelated one added" when reconciling existing call
/// sites' `args` (see `Macro::reconcile_block_call_args`), since a plain
/// name (which *does* change on rename) can't serve as that identity.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum BlockPiece {
    Label { id: String, text: String },
    Input { id: String, name: String },
}

impl BlockPiece {
    fn id(&self) -> &str {
        match self {
            BlockPiece::Label { id, .. } | BlockPiece::Input { id, .. } => id,
        }
    }
}

/// A user-defined custom block ("My Blocks") — just the prototype/signature;
/// the actual instructions it runs live in a `Strand` elsewhere in the same
/// macro whose `instructions[0]` is `Instruction::BlockHeader(id)`, exactly
/// the way `WhenRan` marks an entry strand (see `Macro::run`, which builds
/// its `block_table` by scanning for these header strands directly).
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct BlockDef {
    pub id: String,
    pub pieces: Vec<BlockPiece>,
    pub returns_value: bool,
}

impl BlockDef {
    /// Declared input names, in prototype order — the positional key `Call`/
    /// `CallBlock`'s `args` line up against.
    pub fn input_names(&self) -> impl Iterator<Item = &str> {
        self.pieces.iter().filter_map(|p| match p {
            BlockPiece::Input { name, .. } => Some(name.as_str()),
            BlockPiece::Label { .. } => None,
        })
    }
}

fn default_block_id() -> String {
    Uuid::new_v4().simple().to_string()
}

impl Instruction {
    /// Returns `true` for "header" blocks — blocks that must be first in their
    /// strand, cannot have anything stacked above them, and render with a flat
    /// top edge (no connector notch). `WhenRan` marks a macro entry point;
    /// `BlockHeader` marks a custom block's body.
    pub fn is_header(&self) -> bool {
        matches!(self, Instruction::WhenRan | Instruction::BlockHeader(_))
    }

    /// See `Value::rename_var` — also renames a `SetVariable`/`ChangeVariable`
    /// instruction's own target name, not just `Value::Var` reads nested
    /// inside its value.
    pub fn rename_var(&mut self, old: &str, new: &str) {
        match self {
            Instruction::Wait(value) | Instruction::Return(value) => value.rename_var(old, new),
            Instruction::Token(token) => token.rename_var(old, new),
            Instruction::SetVariable(name, value) | Instruction::ChangeVariable(name, value) => {
                if name == old {
                    *name = new.to_string();
                }
                value.rename_var(old, new);
            }
            Instruction::CallBlock { args, .. } => {
                for a in args.iter_mut() {
                    a.rename_var(old, new);
                }
            }
            Instruction::Command(_) | Instruction::Comment(_) | Instruction::WhenRan | Instruction::BlockHeader(_) => {}
        }
    }

    /// Renames every `Value::Param` leaf reading `old` (anywhere this
    /// instruction embeds a `Value` tree) to `new` — used by
    /// `Macro::rename_block_input` to keep a block's own body working after
    /// one of its inputs is renamed.
    pub fn rename_param(&mut self, old: &str, new: &str) {
        match self {
            Instruction::Wait(value) | Instruction::Return(value) => value.rename_param(old, new),
            Instruction::Token(token) => token.rename_param(old, new),
            Instruction::SetVariable(_, value) | Instruction::ChangeVariable(_, value) => value.rename_param(old, new),
            Instruction::CallBlock { args, .. } => {
                for a in args.iter_mut() {
                    a.rename_param(old, new);
                }
            }
            Instruction::Command(_) | Instruction::Comment(_) | Instruction::WhenRan | Instruction::BlockHeader(_) => {}
        }
    }

    /// Applies `f` to the `args` of every `CallBlock`/`Value::Call` node
    /// (anywhere this instruction embeds one, including nested inside a
    /// `Value` tree) that references `block_id` — used by
    /// `Macro::insert_block_input`/`remove_block_input` to keep every call
    /// site's argument list positionally aligned with the block's current
    /// `pieces` after an input is added/removed.
    pub fn for_each_call_args_mut(&mut self, block_id: &str, f: &mut dyn FnMut(&mut Vec<Value>)) {
        match self {
            Instruction::Wait(value) | Instruction::Return(value) => value.for_each_call_args_mut(block_id, f),
            Instruction::Token(token) => token.for_each_call_args_mut(block_id, f),
            Instruction::SetVariable(_, value) | Instruction::ChangeVariable(_, value) => {
                value.for_each_call_args_mut(block_id, f)
            }
            Instruction::CallBlock { block_id: id, args } => {
                if id == block_id {
                    f(args);
                }
                for a in args.iter_mut() {
                    a.for_each_call_args_mut(block_id, f);
                }
            }
            Instruction::Command(_) | Instruction::Comment(_) | Instruction::WhenRan | Instruction::BlockHeader(_) => {}
        }
    }

    /// Replaces every `Value::Call` node (anywhere this instruction embeds
    /// one) referencing `block_id` with a plain `0` leaf, and drops this
    /// instruction entirely if it's a `CallBlock` referencing `block_id` —
    /// used by `Macro::remove_block` so deleting a custom block scrubs every
    /// reference instead of leaving a dangling one with nothing sensible to
    /// fall back to.
    pub fn scrub_block_calls(&mut self, block_id: &str) {
        match self {
            Instruction::Wait(value) | Instruction::Return(value) => value.scrub_block_calls(block_id),
            Instruction::Token(token) => token.scrub_block_calls(block_id),
            Instruction::SetVariable(_, value) | Instruction::ChangeVariable(_, value) => value.scrub_block_calls(block_id),
            Instruction::CallBlock { args, .. } => {
                for a in args.iter_mut() {
                    a.scrub_block_calls(block_id);
                }
            }
            Instruction::Command(_) | Instruction::Comment(_) | Instruction::WhenRan | Instruction::BlockHeader(_) => {}
        }
    }
}

impl Strand {
    pub fn starts_with_when_ran(&self) -> bool {
        self.instructions.first().map_or(false, Instruction::is_header)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "MacroDe")]
pub struct Macro {
    pub id: String,
    pub name: String,
    pub description: String,
    pub strands: Vec<Strand>,
    /// Strand explicitly chosen (via the canvas's "Set Recording Target"
    /// right-click action) to receive freshly-recorded input. `None` means
    /// no explicit choice has been made yet — falls back to the automatic
    /// "first When Ran strand, else first strand" rule. Deliberately kept
    /// out of the undo/redo stacks (which only snapshot `strands`): it's a
    /// pointer/preference, not an edit to the macro's instructions.
    #[serde(default)]
    pub recording_target: Option<String>,
    /// Playback speed for this macro: every `Wait` instruction's duration is
    /// divided by this when the macro runs, combined with the
    /// global runtime override (see `AppState::global_speed_multiplier`).
    /// 1.0 is normal speed, 2.0 is twice as fast (waits are half as long),
    /// 0.5 is half as fast (waits are twice as long). Clamped to
    /// `SPEED_MULTIPLIER_RANGE` wherever it's set.
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: f64,
    /// Value blocks parked on open canvas — see `FloatingValue`.
    #[serde(default)]
    pub floating_values: Vec<FloatingValue>,
    /// User-declared macro-wide variables — see `VariableDef`.
    #[serde(default)]
    pub variables: Vec<VariableDef>,
    /// User-defined custom blocks ("My Blocks") — see `BlockDef`. Each
    /// def's body lives in its own header strand within `strands`.
    #[serde(default)]
    pub block_defs: Vec<BlockDef>,
}

/// Valid range for both the per-macro and global speed multipliers, enforced
/// wherever either is set from user input.
pub const SPEED_MULTIPLIER_RANGE: std::ops::RangeInclusive<f64> = 0.1..=10.0;

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
        #[serde(default)]
        variables: Vec<VariableDef>,
        #[serde(default)]
        block_defs: Vec<BlockDef>,
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
            MacroDe::Current { id, name, description, mut strands, recording_target, speed_multiplier, floating_values, variables, block_defs } => {
                // Pre-"When Ran" saves have a strand literally id=="root" that
                // was the sole implicit entry point; give it a real WhenRan
                // block so it keeps running after upgrade.
                if let Some(legacy) = strands.iter_mut().find(|s| s.id == LEGACY_ROOT_STRAND_ID) {
                    if !legacy.starts_with_when_ran() {
                        legacy.instructions.insert(0, Instruction::WhenRan);
                    }
                }
                Self { id, name, description, strands, recording_target, speed_multiplier, floating_values, variables, block_defs }
            }
            MacroDe::Legacy { id, name, description, mut code } => {
                code.insert(0, Instruction::WhenRan);
                let strand = Strand { id: default_strand_id(), x: 0, y: 0, instructions: code };
                Self { id, name, description, strands: vec![strand], recording_target: None, speed_multiplier: default_speed_multiplier(), floating_values: Vec::new(), variables: Vec::new(), block_defs: Vec::new() }
            }
        }
    }
}

impl Macro {
    pub fn new(name: String, description: String, mut code: Vec<Instruction>) -> Self {
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
            variables: Vec::new(),
            block_defs: Vec::new(),
        }
    }

    pub fn ensure_id(&mut self) {
        if self.id.trim().is_empty() {
            self.id = default_macro_id();
        }
    }

    pub fn strand(&self, id: &str) -> Option<&Strand> {
        self.strands.iter().find(|s| s.id == id)
    }

    pub fn strand_mut(&mut self, id: &str) -> Option<&mut Strand> {
        self.strands.iter_mut().find(|s| s.id == id)
    }

    pub fn floating_value_mut(&mut self, id: &str) -> Option<&mut FloatingValue> {
        self.floating_values.iter_mut().find(|f| f.id == id)
    }

    /// Writes the live runtime values (`AppState::variable_values`, keyed by
    /// name) back into this macro's `variables` before it's saved to disk —
    /// used once a run finishes/a loop stops, not per-instruction (see the
    /// `macros_thread` module in the tauri app crate).
    pub fn sync_variables_from(&mut self, values: &HashMap<String, Evaluated>) {
        for var in &mut self.variables {
            if let Some(v) = values.get(&var.name) {
                var.value = v.clone();
            }
        }
    }

    /// Renames a declared variable and every existing reference to it —
    /// `Value::Var` reads and `SetVariable`/`ChangeVariable` targets, in
    /// every strand and floating value — so in-progress blocks keep working
    /// under the new name instead of being silently orphaned. A no-op if
    /// `old` isn't a declared variable.
    pub fn rename_variable(&mut self, old: &str, new: &str) {
        if let Some(var) = self.variables.iter_mut().find(|v| v.name == old) {
            var.name = new.to_string();
        } else {
            return;
        }
        for strand in &mut self.strands {
            for ins in &mut strand.instructions {
                ins.rename_var(old, new);
            }
        }
        for fv in &mut self.floating_values {
            fv.value.rename_var(old, new);
        }
    }

    /// Defines a new custom block: appends the `BlockDef` and creates its
    /// (initially empty) header strand at `(x, y)`, returning the new
    /// block's id. Caller validates `pieces` (non-empty label text, unique
    /// input names) beforehand.
    pub fn create_block(&mut self, pieces: Vec<BlockPiece>, returns_value: bool, x: i32, y: i32) -> String {
        let id = default_block_id();
        self.block_defs.push(BlockDef { id: id.clone(), pieces, returns_value });
        self.strands.push(Strand { id: default_strand_id(), x, y, instructions: vec![Instruction::BlockHeader(id.clone())] });
        id
    }

    /// Renames every `Value::Param` leaf reading `old` to `new`, but *only*
    /// within `block_id`'s own body (params are scoped per-block, so no
    /// other block's body is touched) — the body-side half of reconciling a
    /// renamed input; `edit_block` calls this once per renamed piece before
    /// overwriting `BlockDef::pieces` wholesale with the caller's new list
    /// (which is why this doesn't touch `pieces` itself, unlike
    /// `rename_variable`, whose declaration and references live in the same
    /// place). A no-op if `block_id` has no header strand.
    pub fn rename_block_input_body(&mut self, block_id: &str, old: &str, new: &str) {
        for strand in &mut self.strands {
            if matches!(strand.instructions.first(), Some(Instruction::BlockHeader(id)) if id == block_id) {
                for ins in &mut strand.instructions {
                    ins.rename_param(old, new);
                }
            }
        }
    }

    /// Rebuilds every existing call site's `args` (every `CallBlock`/
    /// `Value::Call` referencing `block_id`, wherever it appears) to line up
    /// with `new_pieces`' input order, carrying over each surviving input's
    /// old value by matching `BlockPiece::id` between `old_pieces` and
    /// `new_pieces` (identity survives a rename; a removed input's old value
    /// is simply dropped, an added one gets a fresh `0`) — called by
    /// `edit_block` before it overwrites `BlockDef::pieces`, so `old_pieces`
    /// should be the def's pieces *before* that happens.
    pub fn reconcile_block_call_args(&mut self, block_id: &str, old_pieces: &[BlockPiece], new_pieces: &[BlockPiece]) {
        let old_input_ids: Vec<&str> = old_pieces.iter().filter(|p| matches!(p, BlockPiece::Input { .. })).map(BlockPiece::id).collect();
        let new_input_ids: Vec<&str> = new_pieces.iter().filter(|p| matches!(p, BlockPiece::Input { .. })).map(BlockPiece::id).collect();
        // For each new input slot, which old slot (if any) it carries over from.
        let mapping: Vec<Option<usize>> = new_input_ids.iter().map(|id| old_input_ids.iter().position(|old| old == id)).collect();

        let mut rebuild = |args: &mut Vec<Value>| {
            *args = mapping.iter().map(|old_idx| old_idx.and_then(|i| args.get(i).cloned()).unwrap_or_else(|| Value::number(0.0))).collect();
        };
        for strand in &mut self.strands {
            for ins in &mut strand.instructions {
                ins.for_each_call_args_mut(block_id, &mut rebuild);
            }
        }
        for fv in &mut self.floating_values {
            fv.value.for_each_call_args_mut(block_id, &mut rebuild);
        }
    }

    /// Deletes a custom block entirely: its `BlockDef`, its header strand,
    /// every `CallBlock` instruction that calls it (anywhere), and every
    /// `Value::Call` node that calls it (collapsed to a plain `0` leaf,
    /// same "takes over the slot" spirit as `commands::apply_value_kind`) —
    /// unlike a deleted variable, a dangling block reference has nothing
    /// sensible to fall back to at resolve time, so it's scrubbed rather
    /// than left dangling.
    pub fn remove_block(&mut self, block_id: &str) {
        self.block_defs.retain(|b| b.id != block_id);
        self.strands.retain(|s| !matches!(s.instructions.first(), Some(Instruction::BlockHeader(id)) if id == block_id));
        for strand in &mut self.strands {
            strand.instructions.retain(|ins| !matches!(ins, Instruction::CallBlock { block_id: id, .. } if id == block_id));
            for ins in &mut strand.instructions {
                ins.scrub_block_calls(block_id);
            }
        }
        for fv in &mut self.floating_values {
            fv.value.scrub_block_calls(block_id);
        }
    }

    /// Strand that freshly-recorded input gets appended to: the explicitly
    /// chosen `recording_target` if it still exists, else the first "When
    /// Ran" entry point if one exists, otherwise just the first strand
    /// (creating one if the macro is completely empty) so recorded input
    /// always lands somewhere.
    pub fn recording_target_mut(&mut self) -> &mut Strand {
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
    pub fn recording_target_id(&self) -> Option<String> {
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
pub enum Instruction {
    Token(InputToken),
    Wait(Value),
    Command(String),
    Comment(String),
    /// Marks a strand as an entry point: the strand containing it (always at
    /// index 0 — enforced by every command that can move/insert blocks) runs
    /// as its own concurrent thread when the macro is run. A macro can have
    /// any number of these.
    WhenRan,
    /// `set <name> to <value>` — overwrites the named variable with whatever
    /// `value` evaluates to (number, text, or anything else the tree
    /// resolves down to).
    SetVariable(String, Value),
    /// `change <name> by <value>` — adds `value` to the named variable.
    /// `value` must evaluate to a number (a no-op otherwise); the variable's
    /// own current value is coerced to `0` first if it isn't already
    /// numeric. See `macros::runner::run_instructions`.
    ChangeVariable(String, Value),
    /// Marks a strand as a custom block's body (see `BlockDef`) — the
    /// `String` is the `BlockDef::id` it belongs to. Header-only, like
    /// `WhenRan`; excluded from `Macro::run`'s entry-strand filter so it
    /// never auto-runs, only via `CallBlock`/`Value::Call`.
    BlockHeader(String),
    /// Command-position invocation of a `returns_value == false` custom
    /// block — runs its body inline with `args` bound to its declared
    /// inputs. See `macros::runner`.
    CallBlock { block_id: String, args: Vec<Value> },
    /// Only meaningful inside a `returns_value == true` block's body:
    /// evaluates `Value` and halts that block's execution, handing the
    /// result back to whatever called it (`CallBlock` or `Value::Call`).
    Return(Value),
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
        self.variables.hash(state);
        self.block_defs.hash(state);
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
            Self::SetVariable(n, v)    => { 5u8.hash(state); n.hash(state); v.hash(state); }
            Self::ChangeVariable(n, v) => { 6u8.hash(state); n.hash(state); v.hash(state); }
            Self::BlockHeader(id) => { 7u8.hash(state); id.hash(state); }
            Self::CallBlock { block_id, args } => { 8u8.hash(state); block_id.hash(state); args.hash(state); }
            Self::Return(v) => { 9u8.hash(state); v.hash(state); }
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
    SetVariable(String, Value),
    ChangeVariable(String, Value),
    BlockHeader(String),
    CallBlock { block_id: String, args: Vec<Value> },
    Return(Value),
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
            InstructionDe::SetVariable(n, v) => Instruction::SetVariable(n, v),
            InstructionDe::ChangeVariable(n, v) => Instruction::ChangeVariable(n, v),
            InstructionDe::BlockHeader(id) => Instruction::BlockHeader(id),
            InstructionDe::CallBlock { block_id, args } => Instruction::CallBlock { block_id, args },
            InstructionDe::Return(v) => Instruction::Return(v),
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

    #[test]
    fn rename_variable_renames_declaration_and_every_reference() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![
            Instruction::SetVariable("x".to_string(), Value::number(1.0)),
            Instruction::ChangeVariable("x".to_string(), Value::Var { name: "x".to_string() }),
            Instruction::Token(InputToken::Text(Value::Var { name: "x".to_string() })),
        ]);
        mac.variables.push(VariableDef { name: "x".to_string(), value: Evaluated::Number(0.0) });
        mac.floating_values.push(FloatingValue { id: "f1".into(), x: 0, y: 0, value: Value::Var { name: "x".to_string() } });

        mac.rename_variable("x", "y");

        assert_eq!(mac.variables[0].name, "y");
        let strand = &mac.strands[0];
        assert_eq!(strand.instructions[1], Instruction::SetVariable("y".to_string(), Value::number(1.0)));
        assert_eq!(strand.instructions[2], Instruction::ChangeVariable("y".to_string(), Value::Var { name: "y".to_string() }));
        assert_eq!(strand.instructions[3], Instruction::Token(InputToken::Text(Value::Var { name: "y".to_string() })));
        assert_eq!(mac.floating_values[0].value, Value::Var { name: "y".to_string() });
    }

    #[test]
    fn rename_variable_is_a_no_op_for_undeclared_name() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![Instruction::Token(InputToken::Text(Value::Var { name: "x".to_string() }))]);
        mac.rename_variable("x", "y");
        assert_eq!(mac.strands[0].instructions[1], Instruction::Token(InputToken::Text(Value::Var { name: "x".to_string() })));
    }
}
