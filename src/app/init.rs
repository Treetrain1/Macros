use crate::app::message::Message;
use crate::app::state::{EditorUiState, ExecutionState, MacroLibraryState};
use crate::app::App;
use crate::config;
use crate::macros::runner::make_backend;
use crate::macros::thread_pool::ThreadPool;
use crate::macros::Macro;
use crate::recording;
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::Config;
use cosmic::cosmic_config::ConfigGet;
use std::sync::{Arc, Mutex};
use tracing::warn;

pub(crate) fn build_app(core: Core) -> App {
    App {
        core,
        config: Config::new(crate::config::APP_ID, 1).unwrap(),
        macro_lib: MacroLibraryState::new(),
        execution: ExecutionState {
            emulator: make_backend(),
            thread_pool: ThreadPool::new(),
            is_looping: Arc::new(Mutex::new(false)),
            loop_mode_enabled: false,
        },
        editor_ui: EditorUiState::new(),
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

    // Start the grab thread unconditionally so hotkeys work from launch.
    recording::start_grab_thread();

    // Start the loopback IPC listener so external processes (e.g. a Geode mod
    // running Geometry Dash under Proton) can trigger recording/playback.
    if app.config.get::<bool>("ipc_enabled").unwrap_or(true) {
        let port = app.config.get::<u16>("ipc_port").unwrap_or(47821);
        tokio::spawn(crate::ipc::run_server(port));
    }

    // Load and apply saved hotkey bindings.
    let bindings = config::load_hotkey_bindings(&app.config);
    recording::update_hotkey_table(bindings.clone());
    app.editor_ui.hotkey_bindings = bindings;

    app.update_title()
}

pub(crate) fn add_default_config(_config: &Config) {
    if let Err(err) = Macro::new("New Macro".into(), "description".into(), vec![]).add() {
        warn!("Failed to add default macro: {}", err);
    }
}
