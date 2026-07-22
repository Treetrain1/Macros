use crate::macros::backend::InputBackend;
use crate::macros::runner::VariableStore;
use crate::macros::thread_pool::ThreadPool;
use crate::macros::Macro;
use crate::state::{emit_state_updated, SharedState};
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

impl Macro {
    pub(crate) fn into_loop_task<R: Runtime>(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        loop_flag: Arc<Mutex<bool>>,
        speed_multiplier: f64,
        variables: VariableStore,
        shared_state: SharedState,
        app: AppHandle<R>,
    ) -> impl FnOnce() + Send + 'static {
        move || {
            println!("Starting macro loop: {}", self.name);
            let macro_id = self.id.clone();
            loop {
                if let Ok(should_continue) = loop_flag.lock() {
                    if !*should_continue {
                        break;
                    }
                } else {
                    warn!("Failed to lock loop flag, stopping loop");
                    break;
                }

                self.clone().run(Arc::clone(&emulator), Some(Arc::clone(&loop_flag)), speed_multiplier, Arc::clone(&variables));

                //todo better solution std::thread::sleep(std::time::Duration::from_millis(1));
            }
            persist_variables(&shared_state, &app, &macro_id, &variables);
            println!("Macro loop stopped.");
        }
    }

    pub(crate) fn into_single_run_task<R: Runtime>(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Arc<Mutex<bool>>,
        speed_multiplier: f64,
        variables: VariableStore,
        shared_state: SharedState,
        app: AppHandle<R>,
    ) -> impl FnOnce() + Send + 'static {
        move || {
            println!("Running macro: {}", self.name);
            let macro_id = self.id.clone();
            // Pass None so Wait instructions use the direct spin-sleep path
            // instead of the stoppable polling loop. Single runs don't need
            // mid-wait interruption, and the polling loop was causing waits
            // to be skipped when stop_requested fired spuriously.
            self.run(emulator, None, speed_multiplier, Arc::clone(&variables));
            persist_variables(&shared_state, &app, &macro_id, &variables);
            if let Ok(mut stopped) = stop_flag.lock() {
                *stopped = false;
            }
            println!("Macro complete.");
        }
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
