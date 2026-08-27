use std::sync::{Arc, Mutex};
use tracing::warn;

pub fn set_loop_state(loop_flag: &Arc<Mutex<bool>>, state: bool) -> Result<(), String> {
    match loop_flag.lock() {
        Ok(mut flag) => {
            *flag = state;
            Ok(())
        }
        Err(err) => {
            let error_msg = format!("Failed to set loop state: {}", err);
            warn!("{}", error_msg);
            Err(error_msg)
        }
    }
}

pub fn get_loop_state(loop_flag: &Arc<Mutex<bool>>) -> bool {
    loop_flag.lock().map(|flag| *flag).unwrap_or(false)
}

pub fn stop_loop(loop_flag: &Arc<Mutex<bool>>) -> Result<(), String> {
    set_loop_state(loop_flag, false)
}

pub fn start_loop(loop_flag: &Arc<Mutex<bool>>) -> Result<(), String> {
    set_loop_state(loop_flag, true)
}
