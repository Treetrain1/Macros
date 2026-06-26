use crate::input::types::{Coordinate, Direction, InputToken, MacroKey};
use crate::macros::backend::{create_backend, InputBackend};
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{Instruction, Macro};
use rand::RngExt;
use spin_sleep::{SpinSleeper, SpinStrategy};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;

static EMULATOR_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) fn emulator_failed() -> bool {
    EMULATOR_FAILED.load(Ordering::Relaxed)
}

fn spin_sleeper() -> &'static SpinSleeper {
    static SLEEPER: OnceLock<SpinSleeper> = OnceLock::new();
    SLEEPER.get_or_init(|| SpinSleeper::default().with_spin_strategy(SpinStrategy::SpinLoopHint))
}

/// Polling granularity while waiting for a `Wait` instruction to elapse during
/// a stoppable (loop-mode) run, so a stop signal lands quickly instead of
/// only being noticed once the full wait has elapsed.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(15);

impl Macro {
    /// `stop_flag`, when present, is checked between every instruction (and
    /// while a `Wait` is elapsing) so the run can be aborted early — e.g. when
    /// the "Stop Loop" hotkey is pressed mid-iteration. Either way, any keys
    /// pressed by the macro so far are released before returning, whether the
    /// run finished naturally or was aborted.
    pub(crate) fn run(self, emulator: Arc<Mutex<dyn InputBackend>>, stop_flag: Option<Arc<Mutex<bool>>>) {
        raise_current_thread_priority();
        let mut deadline = Instant::now();
        let mut pressed_keys: Vec<MacroKey> = Vec::new();

        let stop_requested = |flag: &Arc<Mutex<bool>>| -> bool {
            !flag.lock().map(|g| *g).unwrap_or(false)
        };

        let normalize_modifier_key = |key: MacroKey| -> MacroKey {
            match key {
                MacroKey::Shift => MacroKey::LShift,
                _ => key,
            }
        };

        let shift_is_pressed = |keys: &[MacroKey]| -> bool {
            keys.iter()
                .any(|k| matches!(k, MacroKey::Shift | MacroKey::LShift | MacroKey::RShift))
        };

        for ins in self.code {
            if let Some(flag) = &stop_flag {
                if stop_requested(flag) {
                    break;
                }
            }

            #[allow(unreachable_patterns)]
            match ins {
                Instruction::Comment(_) => {}
                Instruction::Wait(duration, randomness) => {
                    let actual = if randomness > 0.0 {
                        let offset = rand::rng().random_range(0.0..=randomness);
                        if rand::random::<bool>() {
                            duration + offset
                        } else {
                            (duration - offset).max(0.0)
                        }
                    } else {
                        duration
                    };
                    deadline += Duration::from_secs_f64(actual / 1000.0);

                    if let Some(flag) = &stop_flag {
                        let mut stopped = false;
                        loop {
                            if stop_requested(flag) {
                                stopped = true;
                                break;
                            }
                            let now = Instant::now();
                            if now >= deadline {
                                break;
                            }
                            let remaining = deadline - now;
                            if remaining > STOP_POLL_INTERVAL {
                                spin_sleeper().sleep(STOP_POLL_INTERVAL);
                            } else {
                                spin_sleeper().sleep_until(deadline);
                                break;
                            }
                        }
                        if stopped {
                            break;
                        }
                    } else {
                        let now = Instant::now();
                        if now >= deadline {
                            deadline = now; // fell behind; re-anchor instead of catching up
                        } else {
                            spin_sleeper().sleep_until(deadline);
                        }
                    }
                }
                Instruction::Command(command) => {
                    println!("Running command: {command}");
                    if let Err(e) = Command::new("bash").args(["-c", &command]).status() {
                        warn!("Command failed: {}", e)
                    }
                }
                Instruction::Token(token) => {
                    let mut em = match emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return;
                        }
                    };

                    match token {
                        InputToken::Text(text) => {
                            if let Err(err) = em.text(&text) {
                                warn!("Failed to type text '{}': {}", text, err);
                            }
                        }
                        InputToken::Key(key, direction) => {
                            let normalized_key = normalize_modifier_key(key);
                            let key_for_event =
                                match (normalized_key.clone(), direction.clone()) {
                                    (MacroKey::Unicode(c), Direction::Click)
                                        if shift_is_pressed(&pressed_keys)
                                            && c.is_ascii_lowercase() =>
                                    {
                                        MacroKey::Unicode(c.to_ascii_uppercase())
                                    }
                                    (key, _) => key,
                                };

                            match em.key(key_for_event, direction.clone()) {
                                Ok(()) => match direction {
                                    Direction::Press => {
                                        if !pressed_keys.contains(&normalized_key) {
                                            pressed_keys.push(normalized_key);
                                        }
                                    }
                                    Direction::Release => {
                                        pressed_keys.retain(|k| k != &normalized_key);
                                    }
                                    Direction::Click => {}
                                },
                                Err(err) => {
                                    warn!(
                                        "Failed to press key {:?} ({:?}): {}",
                                        normalized_key, direction, err
                                    );
                                }
                            }
                        }
                        InputToken::Raw(keycode, direction) => {
                            if let Err(err) = em.raw_keycode(keycode, direction) {
                                warn!("Failed to emit raw keycode {}: {}", keycode, err);
                            }
                        }
                        InputToken::Button(button, direction) => {
                            let dir_clone = direction.clone();
                            if let Err(err) = em.button(button.clone(), direction) {
                                warn!(
                                    "Failed to click button {:?} ({:?}): {}",
                                    button, dir_clone, err
                                );
                            }
                        }
                        InputToken::MoveMouse(x, y, Coordinate::Rel) => {
                            if let Err(err) = em.move_mouse_rel(x, y) {
                                warn!("Failed to move mouse rel ({}, {}): {}", x, y, err);
                            }
                        }
                        InputToken::MoveMouse(x, y, Coordinate::Abs) => {
                            if let Err(err) = em.move_mouse_abs(x, y) {
                                warn!("Failed to move mouse abs ({}, {}): {}", x, y, err);
                            }
                        }
                        InputToken::Scroll(amount, axis) => {
                            if let Err(err) = em.scroll(amount, axis) {
                                warn!("Failed to scroll by {}: {}", amount, err);
                            }
                        }
                    }
                }
                _ => {
                    warn!("Instruction not implemented.");
                }
            }
        }

        if !pressed_keys.is_empty() {
            if let Ok(mut em) = emulator.lock() {
                for key in pressed_keys.into_iter().rev() {
                    if let Err(err) = em.key(key.clone(), Direction::Release) {
                        warn!("Failed to release key {:?} during cleanup: {}", key, err);
                    }
                }
            } else {
                warn!("Failed to lock emulator mutex for key cleanup");
            }
        }
    }
}

pub fn make_backend() -> Option<Arc<Mutex<dyn InputBackend>>> {
    match create_backend() {
        Some(b) => Some(b),
        None => {
            EMULATOR_FAILED.store(true, Ordering::Relaxed);
            None
        }
    }
}
