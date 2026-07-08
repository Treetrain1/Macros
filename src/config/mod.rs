pub(crate) mod persistence;
pub(crate) mod selection;
pub(crate) mod settings;

pub(crate) use persistence::{get_macros_from_config, migrate_legacy_config_dir, migrate_legacy_macros_to_files};
pub(crate) use selection::{get_macro_by_id, get_selected_macro_id, set_selected_macro_id};
pub(crate) use settings::{get_config_value, save_config_value};

pub(crate) const APP_ID: &str = "dev.ethanstokes.Macros";
pub(crate) const GLOBAL_HOTKEYS_KEY: &str = "global_hotkeys";

use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo, MOD_ALT, MOD_CTRL};
use crate::macros::Macro;
use cosmic::cosmic_config::Config;

impl Macro {
    pub(crate) fn add(mut self) -> Result<(), String> {
        self.ensure_id();
        self.save()
    }
}

pub(crate) fn load_hotkey_bindings(config: &Config) -> Vec<HotkeyBinding> {
    if let Some(bindings) = get_config_value::<Vec<HotkeyBinding>>(config, GLOBAL_HOTKEYS_KEY) {
        return bindings;
    }
    default_hotkey_bindings()
}

pub(crate) fn default_combo_for_action(action: &HotkeyAction) -> Option<KeyCombo> {
    default_hotkey_bindings()
        .into_iter()
        .find(|b| &b.action == action)
        .map(|b| b.combo)
}

fn default_hotkey_bindings() -> Vec<HotkeyBinding> {
    vec![
        HotkeyBinding {
            action: HotkeyAction::RunMacro,
            combo: KeyCombo {
                modifiers: MOD_CTRL | MOD_ALT,
                key: "KeyM".to_string(),
            },
        },
        HotkeyBinding {
            action: HotkeyAction::StopLoop,
            combo: KeyCombo {
                modifiers: MOD_CTRL | MOD_ALT,
                key: "KeyS".to_string(),
            },
        },
        HotkeyBinding {
            action: HotkeyAction::NextMacro,
            combo: KeyCombo {
                modifiers: MOD_CTRL | MOD_ALT,
                key: "RightArrow".to_string(),
            },
        },
        HotkeyBinding {
            action: HotkeyAction::PrevMacro,
            combo: KeyCombo {
                modifiers: MOD_CTRL | MOD_ALT,
                key: "LeftArrow".to_string(),
            },
        },
        HotkeyBinding {
            action: HotkeyAction::ToggleLoop,
            combo: KeyCombo {
                modifiers: MOD_CTRL | MOD_ALT,
                key: "KeyL".to_string(),
            },
        },
    ]
}
