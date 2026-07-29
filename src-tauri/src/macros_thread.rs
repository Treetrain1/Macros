use crate::state::{emit_state_updated, SharedState};
use macros_core::macros::backend::InputBackend;
use macros_core::macros::run_registry;
use macros_core::macros::runner::VariableStore;
use macros_core::macros::thread_pool::ThreadPool;
use macros_core::macros::Macro;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Runtime};
use tracing::warn;

/// Writes the run's final variable values back into the (still-selected)
/// macro and saves it to disk — called once when a run finishes or a loop
/// stops, not per-instruction, so a tight loop doesn't hammer the disk. A
/// no-op if the macro was switched away from mid-run.
fn persist_variables<R: Runtime>(shared_state: &SharedState, app: &AppHandle<R>, macro_id: &str, variables: &VariableStore) {
    let Ok(mut s) = shared_state.lock() else { return };
    let Some(mac) = s.current_macro.as_mut() else { return };
    if mac.id != macro_id {
        return;
    }
    if let Ok(values) = variables.lock() {
        mac.sync_variables_from(&values);
    }
    if let Err(e) = mac.save() {
        warn!("Failed to persist variable values: {e}");
    }
    emit_state_updated(app, &s);
}

pub(crate) fn into_loop_task<R: Runtime>(
    mac: Macro,
    emulator: Arc<Mutex<dyn InputBackend>>,
    loop_flag: Arc<Mutex<bool>>,
    speed_multiplier: f64,
    variables: VariableStore,
    shared_state: SharedState,
    app: AppHandle<R>,
) -> impl FnOnce() + Send + 'static {
    move || {
        println!("Starting macro loop: {}", mac.name);
        let macro_id = mac.id.clone();
        run_registry::register(&loop_flag);
        loop {
            if let Ok(should_continue) = loop_flag.lock() {
                if !*should_continue {
                    break;
                }
            } else {
                warn!("Failed to lock loop flag, stopping loop");
                break;
            }

            mac.clone().run(Arc::clone(&emulator), Some(Arc::clone(&loop_flag)), speed_multiplier, Arc::clone(&variables));

            //todo better solution std::thread::sleep(std::time::Duration::from_millis(1));
        }
        run_registry::end_run(&loop_flag);
        persist_variables(&shared_state, &app, &macro_id, &variables);
        println!("Macro loop stopped.");
    }
}

pub(crate) fn into_single_run_task<R: Runtime>(
    mac: Macro,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Arc<Mutex<bool>>,
    speed_multiplier: f64,
    variables: VariableStore,
    shared_state: SharedState,
    app: AppHandle<R>,
) -> impl FnOnce() + Send + 'static {
    move || {
        println!("Running macro: {}", mac.name);
        let macro_id = mac.id.clone();
        // A stop flag of this run's own, not the shared `is_looping` that
        // `stop_flag` refers to: `is_looping` is cleared by whichever run
        // finishes first, which used to stop a concurrently-started run
        // mid-wait — the spurious stop that previously made this path pass
        // `None` and so run uninterruptibly.
        let run_flag = run_registry::begin_run();
        mac.run(emulator, Some(Arc::clone(&run_flag)), speed_multiplier, Arc::clone(&variables));
        run_registry::end_run(&run_flag);
        persist_variables(&shared_state, &app, &macro_id, &variables);
        if let Ok(mut stopped) = stop_flag.lock() {
            *stopped = false;
        }
        println!("Macro complete.");
    }
}

pub(crate) fn spawn_macro_thread<F>(
    thread_pool: &mut ThreadPool,
    name: String,
    task: F,
) -> Result<(), String>
where
    F: FnOnce() + Send + 'static,
{
    let thread_num = thread_pool.workers.len();
    let thread_name = format!("macro_thread_{}: {}", thread_num, name);

    match thread::Builder::new().name(thread_name).spawn(task) {
        Ok(handle) => {
            thread_pool.add_worker(handle);
            thread_pool.cleanup_completed_threads();
            Ok(())
        }
        Err(err) => {
            let error_msg = format!("Failed to spawn thread '{}': {}", name, err);
            warn!("{}", error_msg);
            Err(error_msg)
        }
    }
}
