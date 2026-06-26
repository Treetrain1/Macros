use crate::hotkey_types::HotkeyBinding;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::warn;

#[derive(Serialize, Deserialize, Default, Clone)]
pub(crate) struct AppSettings {
    pub(crate) loop_mode_enabled: Option<bool>,
    pub(crate) ipc_port: Option<u16>,
    pub(crate) ipc_auto_start: Option<bool>,
    pub(crate) global_hotkeys: Option<Vec<HotkeyBinding>>,
    pub(crate) selected_macro_id: Option<String>,
}

fn settings_path() -> Result<PathBuf, String> {
    let mut p = dirs::config_dir().ok_or_else(|| "no config directory".to_string())?;
    p.push(super::APP_ID);
    std::fs::create_dir_all(&p)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;
    p.push("settings.json");
    Ok(p)
}

pub(crate) fn load_settings() -> AppSettings {
    settings_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub(crate) fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    std::fs::write(&tmp, json)
        .map_err(|e| format!("Failed to write settings: {e}"))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("Failed to finalize settings: {e}"))
}

pub(crate) fn update_settings<F: FnOnce(&mut AppSettings)>(f: F) {
    let mut settings = load_settings();
    f(&mut settings);
    if let Err(e) = save_settings(&settings) {
        warn!("Failed to save settings: {e}");
    }
}
