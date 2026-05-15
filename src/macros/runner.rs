use crate::macros::{Instruction, Macro};
use enigo::agent::Token::{Button, Key, MoveMouse, Raw, Scroll, Text};
use enigo::Key as EnigoKey;
use enigo::{Enigo, Keyboard, Mouse};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use tracing::warn;

impl Macro {
    pub(crate) fn run(self, enigo: Arc<Mutex<Enigo>>) {
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
                Instruction::Wait(duration, randomness) => {
                    let actual = if randomness > 0 {
                        use rand::Rng;
                        let offset = rand::thread_rng().gen_range(0..=randomness);
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
                Instruction::Script(script) => {
                    println!("Running script: {script}");
                    Command::new("bash")
                        .arg(&script)
                        .output()
                        .expect(&format!("Failed to run script: {script}"));
                }
                Instruction::Token(token) => {
                    let mut enigo = match enigo.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock enigo mutex: {}", err);
                            return;
                        }
                    };

                    match token {
                        Text(text) => {
                            if let Err(err) = enigo.text(&text) {
                                warn!("Failed to type text '{}': {}", text, err);
                            }
                        }
                        Key(key, direction) => {
                            let normalized_key = normalize_modifier_key(key);
                            let key_for_event = match (normalized_key.clone(), direction) {
                                (EnigoKey::Unicode(c), enigo::Direction::Click) if shift_is_pressed(&pressed_keys) && c.is_ascii_lowercase() => {
                                    EnigoKey::Unicode(c.to_ascii_uppercase())
                                }
                                (key, _) => key,
                            };

                            match enigo.key(key_for_event, direction) {
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
                                    warn!("Failed to type key {:?} ({:?}): {}", normalized_key, direction, err);
                                }
                            }
                        }
                        Raw(keycode, direction) => {
                            if let Err(err) = enigo.raw(keycode, direction) {
                                warn!("Failed to type raw keycode {}: {}", keycode, err);
                            }
                        }
                        Button(button, direction) => {
                            if let Err(err) = enigo.button(button, direction) {
                                warn!("Failed to click mouse button {:?} ({:?}): {}", button, direction, err);
                            }
                        }
                        MoveMouse(x, y, coord) => {
                            if let Err(err) = enigo.move_mouse(x, y, coord) {
                                warn!("Failed to move mouse to ({}, {}): {}", x, y, err);
                            }
                        }
                        Scroll(amount, axis) => {
                            if let Err(err) = enigo.scroll(amount, axis) {
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
            if let Ok(mut enigo) = enigo.lock() {
                for key in pressed_keys.into_iter().rev() {
                    if let Err(err) = enigo.key(key.clone(), enigo::Direction::Release) {
                        warn!("Failed to release key {:?} during macro cleanup: {}", key, err);
                    }
                }
            } else {
                warn!("Failed to lock enigo mutex for macro key cleanup");
            }
        }
    }
}

pub fn make_enigo() -> Enigo<'static> {
    Enigo::new(&enigo::Settings::default()).unwrap()
}
