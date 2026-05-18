use crate::config::{get_macros_from_config, get_selected_macro_id, set_selected_macro_id};
use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use crate::macros::thread_pool::ThreadPool;
use crate::macros::{Instruction, Macro};
use cosmic::cosmic_config::{Config, ConfigGet};
use enigo::Enigo;
use slotmap::{DefaultKey, SecondaryMap, SlotMap};
use std::sync::{Arc, Mutex};
use tracing::warn;

pub(crate) struct MacroLibraryState {
    pub(crate) macro_selected: Option<usize>,
    pub(crate) current_macro: Option<Macro>,
    pub(crate) macros: SlotMap<DefaultKey, Macro>,
    pub(crate) macro_keys: SecondaryMap<DefaultKey, String>,
    pub(crate) macro_strs: Vec<String>,
}

impl MacroLibraryState {
    pub(crate) fn new() -> Self {
        Self {
            macro_selected: None,
            current_macro: None,
            macros: SlotMap::new(),
            macro_keys: SecondaryMap::new(),
            macro_strs: vec![],
        }
    }

    pub(crate) fn update_macro(&mut self, config: &Config, selected: Option<usize>) {
        self.macro_selected = selected;
        let macros = get_macros_from_config(config);

        if let Some(index) = selected {
            if let Some(mac) = macros.get(index).cloned() {
                if let Err(err) = set_selected_macro_id(config, Some(&mac.id)) {
                    warn!("Failed to save selected macro id: {}", err);
                }
                self.current_macro = Some(mac);
                return;
            }
        }

        self.macro_selected = None;
        self.current_macro = None;
        if let Err(err) = set_selected_macro_id(config, None) {
            warn!("Failed to clear selected macro id: {}", err);
        }
    }

    pub(crate) fn update_macros(&mut self, config: &Config) {
        let macs = get_macros_from_config(config);
        self.macros.clear();
        self.macro_keys.clear();
        self.macro_strs.clear();
        for mac in &macs {
            let key = self.macros.insert(mac.clone());
            let mac = self.macros.get_mut(key).unwrap();
            self.macro_keys.insert(key, mac.name.clone());
            self.macro_strs.push(mac.name.clone());
        }

        if let Some(selected_id) = get_selected_macro_id(config) {
            if let Some((index, mac)) = macs
                .iter()
                .enumerate()
                .find(|(_, mac)| mac.id == selected_id)
            {
                self.macro_selected = Some(index);
                self.current_macro = Some(mac.clone());
                return;
            }
        }

        self.macro_selected = None;
        self.current_macro = None;
    }
}

pub(crate) struct ExecutionState {
    pub(crate) enigo: Arc<Mutex<Enigo<'static>>>,
    pub(crate) thread_pool: ThreadPool,
    pub(crate) is_looping: Arc<Mutex<bool>>,
    pub(crate) loop_mode_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RecordingPhase {
    Idle,
    Countdown(u8),
    Active,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Page {
    Main,
    Settings,
}

/// Which hotkey slot is currently being captured via keyboard::listen().
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ComboCapture {
    /// Capturing for a named fixed-action binding.
    Named(HotkeyAction),
    /// Capturing for the "Add per-macro hotkey" form.
    Pending,
}

pub(crate) struct EditorUiState {
    pub(crate) confirm_remove_macro: bool,
    pub(crate) confirm_clear_instructions: bool,
    pub(crate) clear_confirm_generation: u64,
    pub(crate) key_capture_index: Option<usize>,
    pub(crate) undo_stack: Vec<Vec<Instruction>>,
    pub(crate) redo_stack: Vec<Vec<Instruction>>,
    pub(crate) recording_phase: RecordingPhase,
    pub(crate) recording_countdown_generation: u64,
    pub(crate) record_mouse_relative: bool,
    pub(crate) page: Page,
    pub(crate) combo_capture: Option<ComboCapture>,
    pub(crate) hotkey_bindings: Vec<HotkeyBinding>,
    /// (macro index, key combo) for the Add per-macro hotkey form.
    pub(crate) pending_macro_hotkey: Option<(Option<usize>, Option<KeyCombo>)>,
    pub(crate) scroll_offset_y: f32,
    pub(crate) scroll_viewport_height: f32,
}

impl EditorUiState {
    pub(crate) fn new() -> Self {
        Self {
            confirm_remove_macro: false,
            confirm_clear_instructions: false,
            clear_confirm_generation: 0,
            key_capture_index: None,
            undo_stack: vec![],
            redo_stack: vec![],
            recording_phase: RecordingPhase::Idle,
            recording_countdown_generation: 0,
            record_mouse_relative: false,
            page: Page::Main,
            combo_capture: None,
            hotkey_bindings: vec![],
            pending_macro_hotkey: None,
            scroll_offset_y: 0.0,
            scroll_viewport_height: 600.0,
        }
    }

    pub(crate) fn reset_confirms(&mut self) {
        self.confirm_remove_macro = false;
        self.confirm_clear_instructions = false;
        self.key_capture_index = None;
    }

    pub(crate) fn is_capturing_key(&self) -> bool {
        self.key_capture_index.is_some() || self.combo_capture.is_some()
    }
}
