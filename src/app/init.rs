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
            ipc_server: None,
            ipc_shutdown_tx: None,
            ipc_active_port: None,
            ipc_auto_start: false,
        },
        editor_ui: EditorUiState::new(),
    }
}

pub(crate) fn setup_app(app: &mut App) -> Task<Message> {
    config::migrate_legacy_config_dir();

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

    // On macOS the event tap requires Accessibility permission. Prompt for it
    // before starting the grab thread; if it's already granted the call is a
    // no-op. The user must restart the app after granting permission.
    #[cfg(target_os = "macos")]
    {
        let trusted = crate::macros::backend::macos::request_accessibility();
        if !trusted {
            recording::set_grab_failed(true);
        }
    }

    // Start the grab thread unconditionally so hotkeys work from launch.
    recording::start_grab_thread();

    // Start the loopback IPC listener so external processes (e.g. a Geode mod
    // running Geometry Dash under Proton) can trigger recording/playback.
    // Disabled by default; the user opts in via the TCP Server settings section.
    let port = app.config.get::<u16>("ipc_port").unwrap_or(47821);
    app.editor_ui.ipc_port_text = port.to_string();
    app.execution.ipc_auto_start = app.config.get::<bool>("ipc_enabled").unwrap_or(false);
    if app.execution.ipc_auto_start {
        let (tx, rx) = tokio::sync::watch::channel(false);
        app.execution.ipc_server = Some(tokio::spawn(crate::ipc::run_server(port, rx)));
        app.execution.ipc_shutdown_tx = Some(tx);
        app.execution.ipc_active_port = Some(port);
    }

    // Load and apply saved hotkey bindings.
    let bindings = config::load_hotkey_bindings(&app.config);
    recording::update_hotkey_table(bindings.clone());
    app.editor_ui.hotkey_bindings = bindings;

    #[cfg(windows)]
    {
        // Delay slightly so the update check doesn't compete with initial UI rendering.
        let check_task = Task::perform(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let current_version = env!("CARGO_PKG_VERSION").to_string();
                tokio::task::spawn_blocking(move || {
                    crate::updater::check_for_update(&current_version)
                })
                .await
                .unwrap_or_else(|err| Err(format!("Task join error: {err}")))
            },
            |result| Message::UpdateCheckResult(result.map(|opt| opt.map(|info| info.version))).into(),
        );
        return Task::batch(vec![app.update_title(), check_task]);
    }
    #[cfg(not(windows))]
    app.update_title()
}

pub(crate) fn add_default_config(_config: &Config) {
    if let Err(err) = Macro::new("New Macro".into(), "description".into(), vec![]).add() {
        warn!("Failed to add default macro: {}", err);
    }
}
