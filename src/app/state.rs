use crate::config::{get_macros_from_config, get_selected_macro_id, set_selected_macro_id};
use crate::macros::thread_pool::ThreadPool;
use crate::macros::{Instruction, Macro};
use cosmic::cosmic_config::{Config, ConfigGet};
use enigo::Enigo;
#[cfg(not(target_os = "linux"))]
use global_hotkey::GlobalHotKeyManager;
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

pub(crate) struct EditorUiState {
    pub(crate) confirm_remove_macro: bool,
    pub(crate) confirm_clear_instructions: bool,
    pub(crate) clear_confirm_generation: u64,
    pub(crate) key_capture_index: Option<usize>,
    pub(crate) undo_stack: Vec<Vec<Instruction>>,
    pub(crate) redo_stack: Vec<Vec<Instruction>>,
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
        }
    }

    pub(crate) fn reset_confirms(&mut self) {
        self.confirm_remove_macro = false;
        self.confirm_clear_instructions = false;
        self.key_capture_index = None;
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) struct HotkeyState {
    pub(crate) manager: GlobalHotKeyManager,
    pub(crate) run_macro_id: u32,
    pub(crate) stop_loop_id: u32,
}
