use crate::macros::thread_pool::ThreadPool;
use crate::macros::uinput_emulator::UinputEmulator;
use crate::macros::Macro;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::warn;

impl Macro {
    pub(crate) fn into_loop_task(
        self,
        emulator: Arc<Mutex<UinputEmulator>>,
        loop_flag: Arc<Mutex<bool>>,
    ) -> impl FnOnce() + Send + 'static {
        move || {
            println!("Starting macro loop: {}", self.name);
            loop {
                if let Ok(should_continue) = loop_flag.lock() {
                    if !*should_continue {
                        break;
                    }
                } else {
                    warn!("Failed to lock loop flag, stopping loop");
                    break;
                }

                self.clone().run(Arc::clone(&emulator));

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            println!("Macro loop stopped.");
        }
    }

    pub(crate) fn into_single_run_task(
        self,
        emulator: Arc<Mutex<UinputEmulator>>,
    ) -> impl FnOnce() + Send + 'static {
        move || {
            println!("Running macro: {}", self.name);
            self.run(emulator);
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
