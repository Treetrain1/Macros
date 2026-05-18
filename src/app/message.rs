use crate::macros::Instruction;
use cosmic::iced::keyboard;
#[cfg(not(target_os = "linux"))]
use global_hotkey::GlobalHotKeyEvent;

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
    #[cfg(target_os = "linux")]
    StartRecording,
    #[cfg(target_os = "linux")]
    RecordingCountdown(u64),
    #[cfg(target_os = "linux")]
    StopRecording,
    #[cfg(target_os = "linux")]
    ToggleRecordMouseRelative(bool),
    #[cfg(not(target_os = "linux"))]
    GlobalHotkeyEvent(GlobalHotKeyEvent),
}
