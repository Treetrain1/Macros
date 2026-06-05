use crate::macros::uinput_emulator::UinputEmulator;
use crate::macros::{Instruction, Macro};
use enigo::agent::Token::{Button, Key, MoveMouse, Raw, Scroll, Text};
use enigo::Key as EnigoKey;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use rand::RngExt;
use tracing::warn;

static EMULATOR_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) fn emulator_failed() -> bool {
    EMULATOR_FAILED.load(Ordering::Relaxed)
}

impl Macro {
    pub(crate) fn run(self, emulator: Arc<Mutex<UinputEmulator>>) {
        let mut pressed_keys: Vec<EnigoKey> = Vec::new();

        let normalize_modifier_key = |key: EnigoKey| -> EnigoKey {
            match key {
                EnigoKey::Shift => EnigoKey::LShift,
                _ => key,
            }
        };

        let shift_is_pressed = |keys: &[EnigoKey]| -> bool {
            keys.iter().any(|k| matches!(k, EnigoKey::Shift | EnigoKey::LShift | EnigoKey::RShift))
        };

        for ins in self.code {
            #[allow(unreachable_patterns)] match ins {
                Instruction::Comment(_) => {}
                Instruction::Wait(duration, randomness) => {
                    let actual = if randomness > 0 {
                        let offset = rand::rng().random_range(0..=randomness);
                        if rand::random::<bool>() {
                            duration.saturating_add(offset)
                        } else {
                            duration.saturating_sub(offset)
                        }
                    } else {
                        duration
                    };
                    sleep(std::time::Duration::from_millis(actual));
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
                        Text(text) => {
                            if let Err(err) = em.text(&text) {
                                warn!("Failed to type text '{}': {}", text, err);
                            }
                        }
                        Key(key, direction) => {
                            let normalized_key = normalize_modifier_key(key);
                            let key_for_event = match (normalized_key.clone(), direction) {
                                (EnigoKey::Unicode(c), enigo::Direction::Click)
                                    if shift_is_pressed(&pressed_keys) && c.is_ascii_lowercase() =>
                                {
                                    EnigoKey::Unicode(c.to_ascii_uppercase())
                                }
                                (key, _) => key,
                            };

                            match em.key(key_for_event, direction) {
                                Ok(()) => match direction {
                                    enigo::Direction::Press => {
                                        if !pressed_keys.contains(&normalized_key) {
                                            pressed_keys.push(normalized_key);
                                        }
                                    }
                                    enigo::Direction::Release => {
                                        pressed_keys.retain(|k| k != &normalized_key);
                                    }
                                    enigo::Direction::Click => {}
                                },
                                Err(err) => {
                                    warn!("Failed to press key {:?} ({:?}): {}", normalized_key, direction, err);
                                }
                            }
                        }
                        Raw(keycode, direction) => {
                            if let Err(err) = em.raw(keycode, direction) {
                                warn!("Failed to emit raw keycode {}: {}", keycode, err);
                            }
                        }
                        Button(button, direction) => {
                            if let Err(err) = em.button(button, direction) {
                                warn!("Failed to click button {:?} ({:?}): {}", button, direction, err);
                            }
                        }
                        MoveMouse(x, y, coord) => {
                            if let Err(err) = em.move_mouse(x, y, coord) {
                                warn!("Failed to move mouse to ({}, {}): {}", x, y, err);
                            }
                        }
                        Scroll(amount, axis) => {
                            if let Err(err) = em.scroll(amount, axis) {
                                warn!("Failed to scroll by {}: {}", amount, err);
                            }
                        }
                        _ => {
                            warn!("Token not implemented.");
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
                    if let Err(err) = em.key(key.clone(), enigo::Direction::Release) {
                        warn!("Failed to release key {:?} during cleanup: {}", key, err);
                    }
                }
            } else {
                warn!("Failed to lock emulator mutex for key cleanup");
            }
        }
    }
}

pub fn make_emulator() -> Option<UinputEmulator> {
    match UinputEmulator::new() {
        Ok(e) => Some(e),
        Err(err) => {
            warn!("Failed to initialize uinput emulator: {}", err);
            EMULATOR_FAILED.store(true, Ordering::Relaxed);
            None
        }
    }
}
