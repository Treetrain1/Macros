use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use crate::input::types::{Axis, Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::{Op, Value};
use crate::input::{get_mouse_button_names, key_to_string, mouse_button_to_index};
use crate::macros::backend::InputBackend;
use crate::macros::thread_pool::ThreadPool;
use crate::macros::{FloatingValue, Instruction, Macro, Strand};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(crate) type SharedState = Arc<Mutex<AppState>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum FieldId {
    WaitDuration,
    WaitRandomness,
    MoveMouseX,
    MoveMouseY,
    ScrollAmount,
}

impl std::fmt::Display for FieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldId::WaitDuration => write!(f, "WaitDuration"),
            FieldId::WaitRandomness => write!(f, "WaitRandomness"),
            FieldId::MoveMouseX => write!(f, "MoveMouseX"),
            FieldId::MoveMouseY => write!(f, "MoveMouseY"),
            FieldId::ScrollAmount => write!(f, "ScrollAmount"),
        }
    }
}

impl std::str::FromStr for FieldId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "WaitDuration" => Ok(FieldId::WaitDuration),
            "WaitRandomness" => Ok(FieldId::WaitRandomness),
            "MoveMouseX" => Ok(FieldId::MoveMouseX),
            "MoveMouseY" => Ok(FieldId::MoveMouseY),
            "ScrollAmount" => Ok(FieldId::ScrollAmount),
            _ => Err(format!("Unknown FieldId: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RecordingPhase {
    Idle,
    Countdown(u8),
    Active,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UpdateCheckState {
    Idle,
    Checking,
    UpToDate,
    UpdateAvailable(String),
    Applying,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Page {
    Main,
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ComboCapture {
    Named(HotkeyAction),
    Pending,
}

/// One undo/redo checkpoint — everything a "structural" edit command can
/// change: the strand list and the floating value blocks parked on canvas.
#[derive(Debug, Clone)]
pub(crate) struct MacroSnapshot {
    pub(crate) strands: Vec<Strand>,
    pub(crate) floating_values: Vec<FloatingValue>,
}

pub(crate) struct AppState {
    pub(crate) macro_selected: Option<usize>,
    pub(crate) current_macro: Option<Macro>,
    pub(crate) macros_list: Vec<Macro>,
    pub(crate) macro_strs: Vec<String>,
    pub(crate) emulator: Option<Arc<Mutex<dyn InputBackend>>>,
    pub(crate) thread_pool: ThreadPool,
    pub(crate) is_looping: Arc<Mutex<bool>>,
    pub(crate) loop_mode_enabled: bool,
    pub(crate) global_speed_multiplier: f64,
    pub(crate) ipc_server: Option<tauri::async_runtime::JoinHandle<()>>,
    pub(crate) ipc_shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub(crate) ipc_active_port: Option<u16>,
    pub(crate) ipc_auto_start: bool,
    pub(crate) confirm_remove_macro: bool,
    pub(crate) remove_confirm_remaining_secs: u8,
    pub(crate) remove_confirm_generation: u64,
    pub(crate) confirm_clear_instructions: bool,
    pub(crate) clear_confirm_remaining_secs: u8,
    pub(crate) clear_confirm_generation: u64,
    pub(crate) key_capture: Option<(String, usize)>,
    pub(crate) undo_stack: Vec<MacroSnapshot>,
    pub(crate) redo_stack: Vec<MacroSnapshot>,
    pub(crate) recording_phase: RecordingPhase,
    pub(crate) recording_countdown_generation: u64,
    pub(crate) record_mouse_relative: bool,
    pub(crate) page: Page,
    pub(crate) combo_capture: Option<ComboCapture>,
    pub(crate) hotkey_bindings: Vec<HotkeyBinding>,
    pub(crate) pending_macro_hotkey: Option<(Option<usize>, Option<KeyCombo>)>,
    pub(crate) invalid_field_buffers: HashMap<ValueLocation, String>,
    pub(crate) ipc_port_text: String,
    pub(crate) ipc_port_invalid: bool,
    pub(crate) update_check_state: UpdateCheckState,
}

// ─── Serializable DTO ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub(crate) struct StateDto {
    pub(crate) macro_names: Vec<String>,
    pub(crate) macro_selected: Option<usize>,
    pub(crate) current_macro: Option<MacroDto>,
    pub(crate) macros_data: Vec<MacroDto>,
    pub(crate) loop_mode_enabled: bool,
    pub(crate) global_speed_multiplier: f64,
    pub(crate) is_looping: bool,
    pub(crate) ipc_active_port: Option<u16>,
    pub(crate) ipc_auto_start: bool,
    pub(crate) confirm_remove_macro: bool,
    pub(crate) confirm_remove_macro_remaining_secs: u8,
    pub(crate) confirm_clear_instructions: bool,
    pub(crate) confirm_clear_instructions_remaining_secs: u8,
    pub(crate) key_capture: Option<KeyCaptureDto>,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) recording_phase: RecordingPhaseDto,
    pub(crate) record_mouse_relative: bool,
    pub(crate) page: String,
    pub(crate) combo_capture: Option<ComboCaptureDto>,
    pub(crate) hotkey_bindings: Vec<HotkeyBindingDto>,
    pub(crate) named_hotkey_defaults: Vec<NamedHotkeyDefaultDto>,
    pub(crate) pending_macro_hotkey: Option<PendingMacroHotkeyDto>,
    pub(crate) invalid_field_buffers: Vec<InvalidFieldDto>,
    pub(crate) ipc_port_text: String,
    pub(crate) ipc_port_invalid: bool,
    pub(crate) emulator_available: bool,
    pub(crate) grab_available: bool,
    pub(crate) update_check_state: UpdateCheckStateDto,
}

#[derive(Serialize, Clone)]
pub(crate) struct MacroDto {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) strands: Vec<StrandDto>,
    pub(crate) recording_target_strand_id: Option<String>,
    pub(crate) speed_multiplier: f64,
    pub(crate) floating_values: Vec<FloatingValueDto>,
}

#[derive(Serialize, Clone)]
pub(crate) struct StrandDto {
    pub(crate) id: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) instructions: Vec<InstructionDto>,
}

#[derive(Serialize, Clone)]
pub(crate) struct KeyCaptureDto {
    pub(crate) strand_id: String,
    pub(crate) index: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub(crate) enum ValueDto {
    Number { value: f64 },
    Text { value: String },
    BinaryOp { op: String, lhs: Box<ValueDto>, rhs: Box<ValueDto>, saved: Box<ValueDto> },
}

#[derive(Serialize, Clone)]
pub(crate) struct FloatingValueDto {
    pub(crate) id: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) value: ValueDto,
}

/// Addresses a single `Value` node — either inside an instruction's field
/// (`Field`) or inside a value block parked on canvas (`Floating`), at
/// `path` within that root (`Value::get_mut`'s `0`=lhs/`1`=rhs addressing).
/// Resolved against a `Macro` by `commands::resolve_location_mut`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ValueLocation {
    Field { strand_id: String, index: usize, field_id: FieldId, path: Vec<u8> },
    Floating { floating_id: String, path: Vec<u8> },
}

impl ValueLocation {
    pub(crate) fn path(&self) -> &[u8] {
        match self {
            ValueLocation::Field { path, .. } => path,
            ValueLocation::Floating { path, .. } => path,
        }
    }

    /// True if `self` and `other` address a node in the same tree (ignoring
    /// `path`) — used to prune stale invalid-text buffers for descendants of
    /// a subtree that just got replaced wholesale.
    pub(crate) fn same_root(&self, other: &ValueLocation) -> bool {
        match (self, other) {
            (
                ValueLocation::Field { strand_id: s1, index: i1, field_id: f1, .. },
                ValueLocation::Field { strand_id: s2, index: i2, field_id: f2, .. },
            ) => s1 == s2 && i1 == i2 && f1 == f2,
            (ValueLocation::Floating { floating_id: a, .. }, ValueLocation::Floating { floating_id: b, .. }) => a == b,
            _ => false,
        }
    }

    /// `Some(strand_id)` for a `Field` location, `None` for `Floating` —
    /// used to prune buffered entries when a whole strand is removed.
    pub(crate) fn strand_id(&self) -> Option<&str> {
        match self {
            ValueLocation::Field { strand_id, .. } => Some(strand_id),
            ValueLocation::Floating { .. } => None,
        }
    }
}

/// Wire shape for `ValueLocation` — received from the frontend as command
/// params, and sent back out as part of `InvalidFieldDto`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub(crate) enum ValueLocationDto {
    Field { strand_id: String, index: usize, field_id: String, path: Vec<u8> },
    Floating { floating_id: String, path: Vec<u8> },
}

impl ValueLocationDto {
    pub(crate) fn to_location(&self) -> Result<ValueLocation, String> {
        Ok(match self {
            ValueLocationDto::Field { strand_id, index, field_id, path } => ValueLocation::Field {
                strand_id: strand_id.clone(),
                index: *index,
                field_id: field_id.parse()?,
                path: path.clone(),
            },
            ValueLocationDto::Floating { floating_id, path } => {
                ValueLocation::Floating { floating_id: floating_id.clone(), path: path.clone() }
            }
        })
    }
}

pub(crate) fn location_to_dto(loc: &ValueLocation) -> ValueLocationDto {
    match loc {
        ValueLocation::Field { strand_id, index, field_id, path } => ValueLocationDto::Field {
            strand_id: strand_id.clone(),
            index: *index,
            field_id: field_id.to_string(),
            path: path.clone(),
        },
        ValueLocation::Floating { floating_id, path } => {
            ValueLocationDto::Floating { floating_id: floating_id.clone(), path: path.clone() }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub(crate) enum InstructionDto {
    Wait { duration: ValueDto, randomness: ValueDto },
    Text { text: String },
    Key { key: String, direction: String },
    Button { button: String, direction: String },
    MoveMouse { x: ValueDto, y: ValueDto, coordinate: String },
    Scroll { amount: ValueDto, axis: String },
    Command { command: String },
    Comment { comment: String },
    WhenRan,
}

#[derive(Serialize, Clone)]
pub(crate) struct RecordingPhaseDto {
    pub(crate) phase: String,
    pub(crate) countdown: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub(crate) enum HotkeyActionDto {
    RunMacro,
    StopLoop,
    NextMacro,
    PrevMacro,
    ToggleLoop,
    StartRecordingImmediate,
    StopRecording,
    Undo,
    Redo,
    RunSpecificMacro { macro_id: String },
}

#[derive(Serialize, Clone)]
pub(crate) struct HotkeyBindingDto {
    pub(crate) binding_index: usize,
    pub(crate) action: HotkeyActionDto,
    pub(crate) combo_display: String,
    pub(crate) macro_name: Option<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct NamedHotkeyDefaultDto {
    pub(crate) action: HotkeyActionDto,
    pub(crate) combo_display: Option<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct ComboCaptureDto {
    pub(crate) kind: String,
    pub(crate) action: Option<HotkeyActionDto>,
}

#[derive(Serialize, Clone)]
pub(crate) struct PendingMacroHotkeyDto {
    pub(crate) macro_index: Option<usize>,
    pub(crate) combo_display: Option<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct InvalidFieldDto {
    pub(crate) location: ValueLocationDto,
    pub(crate) text: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct UpdateCheckStateDto {
    pub(crate) state: String,
    pub(crate) version: Option<String>,
    pub(crate) error: Option<String>,
}

// ─── Conversions ───────────────────────────────────────────────────────────

fn direction_to_str(d: &Direction) -> &'static str {
    match d {
        Direction::Click => "Click",
        Direction::Press => "Press",
        Direction::Release => "Release",
    }
}

fn str_to_direction(s: &str) -> Direction {
    match s {
        "Press" => Direction::Press,
        "Release" => Direction::Release,
        _ => Direction::Click,
    }
}

fn coordinate_to_str(c: &Coordinate) -> &'static str {
    match c {
        Coordinate::Abs => "Absolute",
        Coordinate::Rel => "Relative",
    }
}

fn str_to_coordinate(s: &str) -> Coordinate {
    match s {
        "Relative" => Coordinate::Rel,
        _ => Coordinate::Abs,
    }
}

fn axis_to_str(a: &Axis) -> &'static str {
    match a {
        Axis::Vertical => "Vertical",
        Axis::Horizontal => "Horizontal",
    }
}

fn str_to_axis(s: &str) -> Axis {
    match s {
        "Horizontal" => Axis::Horizontal,
        _ => Axis::Vertical,
    }
}

fn op_to_str(op: &Op) -> &'static str {
    match op {
        Op::Add => "Add",
        Op::Sub => "Sub",
        Op::Mul => "Mul",
        Op::Div => "Div",
    }
}

fn str_to_op(s: &str) -> Op {
    match s {
        "Sub" => Op::Sub,
        "Mul" => Op::Mul,
        "Div" => Op::Div,
        _ => Op::Add,
    }
}

pub(crate) fn value_to_dto(value: &Value) -> ValueDto {
    match value {
        Value::Number { value } => ValueDto::Number { value: *value },
        Value::Text { value } => ValueDto::Text { value: value.clone() },
        Value::BinaryOp { op, lhs, rhs, saved } => ValueDto::BinaryOp {
            op: op_to_str(op).to_string(),
            lhs: Box::new(value_to_dto(lhs)),
            rhs: Box::new(value_to_dto(rhs)),
            saved: Box::new(value_to_dto(saved)),
        },
    }
}

pub(crate) fn dto_to_value(dto: &ValueDto) -> Value {
    match dto {
        ValueDto::Number { value } => Value::Number { value: *value },
        ValueDto::Text { value } => Value::Text { value: value.clone() },
        ValueDto::BinaryOp { op, lhs, rhs, saved } => Value::BinaryOp {
            op: str_to_op(op),
            lhs: Box::new(dto_to_value(lhs)),
            rhs: Box::new(dto_to_value(rhs)),
            saved: Box::new(dto_to_value(saved)),
        },
    }
}

pub(crate) fn instruction_to_dto(ins: &Instruction) -> InstructionDto {
    match ins {
        Instruction::Wait(dur, rand) => InstructionDto::Wait { duration: value_to_dto(dur), randomness: value_to_dto(rand) },
        Instruction::Command(cmd) => InstructionDto::Command { command: cmd.clone() },
        Instruction::Comment(c) => InstructionDto::Comment { comment: c.clone() },
        Instruction::WhenRan => InstructionDto::WhenRan,
        Instruction::Token(token) => match token {
            InputToken::Text(t) => InstructionDto::Text { text: t.clone() },
            InputToken::Key(k, d) => InstructionDto::Key {
                key: key_to_string(k).unwrap_or("Unknown").to_string(),
                direction: direction_to_str(d).to_string(),
            },
            InputToken::Button(b, d) => InstructionDto::Button {
                button: get_mouse_button_names()[mouse_button_to_index(b)].to_string(),
                direction: direction_to_str(d).to_string(),
            },
            InputToken::MoveMouse(x, y, coord) => InstructionDto::MoveMouse {
                x: value_to_dto(x),
                y: value_to_dto(y),
                coordinate: coordinate_to_str(coord).to_string(),
            },
            InputToken::Scroll(amt, axis) => InstructionDto::Scroll {
                amount: value_to_dto(amt),
                axis: axis_to_str(axis).to_string(),
            },
            InputToken::Raw(_, _) => InstructionDto::Comment { comment: "(raw keycode)".to_string() },
        },
    }
}

pub(crate) fn dto_to_instruction(dto: &InstructionDto) -> Option<Instruction> {
    use crate::input::{index_to_mouse_button, key_names::string_to_key};
    Some(match dto {
        InstructionDto::Wait { duration, randomness } => Instruction::Wait(dto_to_value(duration), dto_to_value(randomness)),
        InstructionDto::Text { text } => Instruction::Token(InputToken::Text(text.clone())),
        InstructionDto::Key { key, direction } => {
            let mk = string_to_key(key).ok()?;
            Instruction::Token(InputToken::Key(mk, str_to_direction(direction)))
        }
        InstructionDto::Button { button, direction } => {
            let names = get_mouse_button_names();
            let idx = names.iter().position(|&n| n == button.as_str()).unwrap_or(0);
            Instruction::Token(InputToken::Button(index_to_mouse_button(idx), str_to_direction(direction)))
        }
        InstructionDto::MoveMouse { x, y, coordinate } => {
            Instruction::Token(InputToken::MoveMouse(dto_to_value(x), dto_to_value(y), str_to_coordinate(coordinate)))
        }
        InstructionDto::Scroll { amount, axis } => {
            Instruction::Token(InputToken::Scroll(dto_to_value(amount), str_to_axis(axis)))
        }
        InstructionDto::Command { command } => Instruction::Command(command.clone()),
        InstructionDto::Comment { comment } => Instruction::Comment(comment.clone()),
        InstructionDto::WhenRan => Instruction::WhenRan,
    })
}

fn strand_to_dto(strand: &Strand) -> StrandDto {
    StrandDto {
        id: strand.id.clone(),
        x: strand.x,
        y: strand.y,
        instructions: strand.instructions.iter().map(instruction_to_dto).collect(),
    }
}

fn floating_value_to_dto(fv: &FloatingValue) -> FloatingValueDto {
    FloatingValueDto { id: fv.id.clone(), x: fv.x, y: fv.y, value: value_to_dto(&fv.value) }
}

fn macro_to_dto(mac: &Macro) -> MacroDto {
    MacroDto {
        id: mac.id.clone(),
        name: mac.name.clone(),
        description: mac.description.clone(),
        strands: mac.strands.iter().map(strand_to_dto).collect(),
        recording_target_strand_id: mac.recording_target_id(),
        speed_multiplier: mac.speed_multiplier,
        floating_values: mac.floating_values.iter().map(floating_value_to_dto).collect(),
    }
}

fn hotkey_action_to_dto(action: &HotkeyAction) -> HotkeyActionDto {
    match action {
        HotkeyAction::RunMacro => HotkeyActionDto::RunMacro,
        HotkeyAction::StopLoop => HotkeyActionDto::StopLoop,
        HotkeyAction::NextMacro => HotkeyActionDto::NextMacro,
        HotkeyAction::PrevMacro => HotkeyActionDto::PrevMacro,
        HotkeyAction::ToggleLoop => HotkeyActionDto::ToggleLoop,
        HotkeyAction::StartRecordingImmediate => HotkeyActionDto::StartRecordingImmediate,
        HotkeyAction::StopRecording => HotkeyActionDto::StopRecording,
        HotkeyAction::Undo => HotkeyActionDto::Undo,
        HotkeyAction::Redo => HotkeyActionDto::Redo,
        HotkeyAction::RunSpecificMacro(id) => HotkeyActionDto::RunSpecificMacro { macro_id: id.clone() },
    }
}

pub(crate) fn dto_to_hotkey_action(dto: &HotkeyActionDto) -> HotkeyAction {
    match dto {
        HotkeyActionDto::RunMacro => HotkeyAction::RunMacro,
        HotkeyActionDto::StopLoop => HotkeyAction::StopLoop,
        HotkeyActionDto::NextMacro => HotkeyAction::NextMacro,
        HotkeyActionDto::PrevMacro => HotkeyAction::PrevMacro,
        HotkeyActionDto::ToggleLoop => HotkeyAction::ToggleLoop,
        HotkeyActionDto::StartRecordingImmediate => HotkeyAction::StartRecordingImmediate,
        HotkeyActionDto::StopRecording => HotkeyAction::StopRecording,
        HotkeyActionDto::Undo => HotkeyAction::Undo,
        HotkeyActionDto::Redo => HotkeyAction::Redo,
        HotkeyActionDto::RunSpecificMacro { macro_id } => HotkeyAction::RunSpecificMacro(macro_id.clone()),
    }
}

pub(crate) fn build_state_dto(s: &AppState) -> StateDto {
    let current_macro = s.current_macro.as_ref().map(macro_to_dto);

    let recording_phase = match &s.recording_phase {
        RecordingPhase::Idle => RecordingPhaseDto { phase: "Idle".to_string(), countdown: None },
        RecordingPhase::Countdown(n) => RecordingPhaseDto { phase: "Countdown".to_string(), countdown: Some(*n) },
        RecordingPhase::Active => RecordingPhaseDto { phase: "Active".to_string(), countdown: None },
    };

    let combo_capture = s.combo_capture.as_ref().map(|cc| match cc {
        ComboCapture::Named(action) => ComboCaptureDto {
            kind: "Named".to_string(),
            action: Some(hotkey_action_to_dto(action)),
        },
        ComboCapture::Pending => ComboCaptureDto {
            kind: "Pending".to_string(),
            action: None,
        },
    });

    let macros_data: Vec<MacroDto> = s.macros_list.iter().map(macro_to_dto).collect();

    let macros_list = &s.macros_list;
    let hotkey_bindings: Vec<HotkeyBindingDto> = s.hotkey_bindings.iter().enumerate().map(|(i, b)| {
        let macro_name = if let HotkeyAction::RunSpecificMacro(ref id) = b.action {
            macros_list.iter().find(|m| &m.id == id).map(|m| m.name.clone())
                .or_else(|| Some("(deleted)".to_string()))
        } else {
            None
        };
        HotkeyBindingDto {
            binding_index: i,
            action: hotkey_action_to_dto(&b.action),
            combo_display: b.combo.format(),
            macro_name,
        }
    }).collect();

    const NAMED_HOTKEY_ACTIONS: [HotkeyAction; 9] = [
        HotkeyAction::RunMacro,
        HotkeyAction::StopLoop,
        HotkeyAction::NextMacro,
        HotkeyAction::PrevMacro,
        HotkeyAction::ToggleLoop,
        HotkeyAction::StartRecordingImmediate,
        HotkeyAction::StopRecording,
        HotkeyAction::Undo,
        HotkeyAction::Redo,
    ];
    let named_hotkey_defaults: Vec<NamedHotkeyDefaultDto> = NAMED_HOTKEY_ACTIONS.iter().map(|action| {
        NamedHotkeyDefaultDto {
            action: hotkey_action_to_dto(action),
            combo_display: crate::config::default_combo_for_action(action).map(|c| c.format()),
        }
    }).collect();

    let pending_macro_hotkey = s.pending_macro_hotkey.as_ref().map(|(idx, combo)| PendingMacroHotkeyDto {
        macro_index: *idx,
        combo_display: combo.as_ref().map(|c| c.format()),
    });

    let invalid_field_buffers: Vec<InvalidFieldDto> = s.invalid_field_buffers.iter().map(|(location, text)| {
        InvalidFieldDto { location: location_to_dto(location), text: text.clone() }
    }).collect();

    let key_capture = s.key_capture.as_ref().map(|(strand_id, index)| KeyCaptureDto {
        strand_id: strand_id.clone(),
        index: *index,
    });

    let is_looping = s.is_looping.lock().map(|g| *g).unwrap_or(false);

    let update_check_state = match &s.update_check_state {
        UpdateCheckState::Idle => UpdateCheckStateDto { state: "Idle".to_string(), version: None, error: None },
        UpdateCheckState::Checking => UpdateCheckStateDto { state: "Checking".to_string(), version: None, error: None },
        UpdateCheckState::UpToDate => UpdateCheckStateDto { state: "UpToDate".to_string(), version: None, error: None },
        UpdateCheckState::UpdateAvailable(v) => UpdateCheckStateDto { state: "UpdateAvailable".to_string(), version: Some(v.clone()), error: None },
        UpdateCheckState::Applying => UpdateCheckStateDto { state: "Applying".to_string(), version: None, error: None },
        UpdateCheckState::Error(e) => UpdateCheckStateDto { state: "Error".to_string(), version: None, error: Some(e.clone()) },
    };

    StateDto {
        macro_names: s.macro_strs.clone(),
        macro_selected: s.macro_selected,
        current_macro,
        macros_data,
        loop_mode_enabled: s.loop_mode_enabled,
        global_speed_multiplier: s.global_speed_multiplier,
        is_looping,
        ipc_active_port: s.ipc_active_port,
        ipc_auto_start: s.ipc_auto_start,
        confirm_remove_macro: s.confirm_remove_macro,
        confirm_remove_macro_remaining_secs: s.remove_confirm_remaining_secs,
        confirm_clear_instructions: s.confirm_clear_instructions,
        confirm_clear_instructions_remaining_secs: s.clear_confirm_remaining_secs,
        key_capture,
        can_undo: !s.undo_stack.is_empty(),
        can_redo: !s.redo_stack.is_empty(),
        recording_phase,
        record_mouse_relative: s.record_mouse_relative,
        page: match s.page { Page::Main => "Main".to_string(), Page::Settings => "Settings".to_string() },
        combo_capture,
        hotkey_bindings,
        named_hotkey_defaults,
        pending_macro_hotkey,
        invalid_field_buffers,
        ipc_port_text: s.ipc_port_text.clone(),
        ipc_port_invalid: s.ipc_port_invalid,
        emulator_available: s.emulator.is_some(),
        grab_available: !crate::recording::grab_failed(),
        update_check_state,
    }
}

pub(crate) fn emit_state_updated<R: tauri::Runtime>(app: &tauri::AppHandle<R>, s: &AppState) {
    use tauri::Emitter;
    let dto = build_state_dto(s);
    let _ = app.emit("state-updated", dto);
}
