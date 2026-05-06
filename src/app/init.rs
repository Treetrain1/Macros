use std::sync::{Arc, Mutex};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::ConfigGet;
use cosmic::cosmic_config::Config;
use slotmap::{SecondaryMap, SlotMap};
use tracing::warn;
use crate::app::App;
use crate::app::message::Message;
use crate::app::state::{EditorUiState, ExecutionState, MacroLibraryState};
use crate::config;
use crate::macros::Macro;
use crate::macros::runner::make_enigo;
use crate::macros::thread_pool::ThreadPool;

pub(crate) fn build_app(core: Core) -> App {
    #[cfg(not(target_os = "linux"))]
    let hotkey_state = crate::app::hotkeys::non_linux::setup_non_linux_hotkeys();

    App {
        core,
        config: Config::new(crate::config::APP_ID, 1).unwrap(),
        macro_lib: MacroLibraryState::new(),
        execution: ExecutionState {
            enigo: Arc::new(Mutex::from(make_enigo())),
            thread_pool: ThreadPool::new(),
            is_looping: Arc::new(Mutex::new(false)),
            loop_mode_enabled: false,
        },
        editor_ui: EditorUiState::new(),
        #[cfg(not(target_os = "linux"))]
        hotkey_state,
    }
}

pub(crate) fn setup_app(app: &mut App) -> Task<Message> {
    if let Err(err) = config::migrate_legacy_macros_to_files(&app.config) {
        warn!("Failed to migrate legacy macros: {}", err);
    }

    let macros = config::get_macros_from_config(&app.config);
    if macros.is_empty() {
        add_default_config(&app.config);
    }

    let config = app.config.clone();
    app.macro_lib.update_macros(&config);

    if let Ok(loop_mode) = app.config.get::<bool>("loop_mode_enabled") {
        app.execution.loop_mode_enabled = loop_mode;
    }

    #[cfg(target_os = "linux")]
    crate::app::hotkeys::linux::setup_linux_hotkeys(
        Arc::clone(&app.execution.enigo),
        app.config.clone(),
        Arc::clone(&app.execution.is_looping),
    );

    app.update_title()
}

pub(crate) fn add_default_config(_config: &Config) {
    if let Err(err) = Macro::new("New Macro".into(), "description".into(), vec![]).add() {
        warn!("Failed to add default macro: {}", err);
    }
}
