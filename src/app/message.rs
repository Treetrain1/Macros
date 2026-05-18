use crate::hotkey_types::{HotkeyAction, KeyCombo};
use crate::macros::Instruction;
use cosmic::iced::keyboard;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Message {
    SelectMacro(usize),
    RunMacro,
    ToggleLoopMode(bool),
    SetTitle(String),
    SetDescription(String),
    AddInstruction(usize, Instruction),
    EditInstruction(usize, Instruction),
    StartKeyCapture(usize),
    KeyCaptureEvent(keyboard::Event),
    RemoveInstruction(isize),
    /// Reorder an instruction by moving it up or down.
    /// Parameters: (index, direction) where direction is -1 for up, 1 for down
    ReorderInstruction(usize, isize),
    ClearInstructions,
    ClearInstructionsTimeout(u64),
    SaveMacro,
    NewMacro,
    RemoveMacro,
    Undo,
    Redo,
    StartRecording,
    RecordingCountdown(u64),
    StopRecording,
    ToggleRecordMouseRelative(bool),
    // Settings page
    OpenSettings,
    CloseSettings,
    StartComboCapture(HotkeyAction),
    StartPendingComboCapture,
    SaveHotkeyBindings,
    SetPendingMacroIdx(Option<usize>),
    AddMacroHotkey,
    RemoveHotkeyBinding(usize),
    ClearNamedHotkey(HotkeyAction),
    ResetHotkeyToDefault(HotkeyAction),
    ExecuteHotkeyAction(HotkeyAction),
}
