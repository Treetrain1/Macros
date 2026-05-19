use crate::hotkey_types::{HotkeyAction, HotkeyBinding, MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
use crate::input::rdev_mapping::{map_rdev_button, map_rdev_key};
use crate::macros::Instruction;
use enigo::agent::Token;
use enigo::{Axis, Coordinate, Direction};
use rdev::{EventType, Key as RdevKey};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Instant;
use tracing::warn;

pub(crate) static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) static RECORD_MOUSE_RELATIVE: AtomicBool = AtomicBool::new(false);

static RECORDING_QUEUE: OnceLock<Mutex<VecDeque<Instruction>>> = OnceLock::new();
static STOP_SIGNAL: OnceLock<Mutex<VecDeque<()>>> = OnceLock::new();
static LAST_EVENT_TIME: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static TIMING_REMAINDER_US: OnceLock<Mutex<u64>> = OnceLock::new();
static LAST_MOUSE_POS: OnceLock<Mutex<Option<(f64, f64)>>> = OnceLock::new();

static HOTKEY_TABLE: OnceLock<RwLock<Vec<HotkeyBinding>>> = OnceLock::new();
static HOTKEY_ACTION_QUEUE: OnceLock<Mutex<VecDeque<HotkeyAction>>> = OnceLock::new();

pub(crate) fn get_recording_queue() -> &'static Mutex<VecDeque<Instruction>> {
    RECORDING_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(crate) fn get_stop_signal() -> &'static Mutex<VecDeque<()>> {
    STOP_SIGNAL.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(crate) fn get_hotkey_action_queue() -> &'static Mutex<VecDeque<HotkeyAction>> {
    HOTKEY_ACTION_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(crate) fn update_hotkey_table(bindings: Vec<HotkeyBinding>) {
    let table = HOTKEY_TABLE.get_or_init(|| RwLock::new(vec![]));
    if let Ok(mut t) = table.write() {
        *t = bindings;
    }
}

pub(crate) fn reset_timing() {
    if let Ok(mut t) = LAST_EVENT_TIME.get_or_init(|| Mutex::new(None)).lock() {
        *t = None;
    }
    if let Ok(mut r) = TIMING_REMAINDER_US.get_or_init(|| Mutex::new(0)).lock() {
        *r = 0;
    }
    if let Ok(mut p) = LAST_MOUSE_POS.get_or_init(|| Mutex::new(None)).lock() {
        *p = None;
    }
}

fn modifier_bit(key: &RdevKey) -> u8 {
    match key {
        RdevKey::ControlLeft | RdevKey::ControlRight => MOD_CTRL,
        RdevKey::ShiftLeft | RdevKey::ShiftRight => MOD_SHIFT,
        RdevKey::Alt | RdevKey::AltGr => MOD_ALT,
        RdevKey::MetaLeft | RdevKey::MetaRight => MOD_META,
        _ => 0,
    }
}

fn check_hotkey(mods: u8, key_name: &str) -> Option<HotkeyAction> {
    let table = HOTKEY_TABLE.get_or_init(|| RwLock::new(vec![]));
    if let Ok(bindings) = table.try_read() {
        for binding in bindings.iter() {
            if binding.combo.modifiers == mods && binding.combo.key == key_name {
                return Some(binding.action.clone());
            }
        }
    }
    None
}

pub(crate) fn start_grab_thread() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("rdev-grab".into())
            .spawn(|| {
                let held_mods = AtomicU8::new(0);
                let result = rdev::grab(move |event| {
                    // Track modifier state
                    match &event.event_type {
                        EventType::KeyPress(key) => {
                            let bit = modifier_bit(key);
                            if bit != 0 {
                                held_mods.fetch_or(bit, Ordering::Relaxed);
                            }
                        }
                        EventType::KeyRelease(key) => {
                            let bit = modifier_bit(key);
                            if bit != 0 {
                                held_mods.fetch_and(!bit, Ordering::Relaxed);
                            }
                        }
                        _ => {}
                    }

                    if RECORDING_ACTIVE.load(Ordering::Relaxed) {
                        if matches!(event.event_type, EventType::KeyPress(RdevKey::Escape)) {
                            RECORDING_ACTIVE.store(false, Ordering::Relaxed);
                            if let Ok(mut q) = get_stop_signal().lock() {
                                q.push_back(());
                            }
                            return None;
                        }

                        let last_time = LAST_EVENT_TIME.get_or_init(|| Mutex::new(None));
                        let now = Instant::now();
                        let instr = event_to_instruction(&event.event_type);
                        if let Some(instr) = instr {
                            if let Ok(mut last) = last_time.lock() {
                                let prev = *last;
                                *last = Some(now);
                                if let Ok(mut q) = get_recording_queue().lock() {
                                    if let Some(prev_time) = prev {
                                        let elapsed_us =
                                            now.duration_since(prev_time).as_micros() as u64;
                                        let remainder = TIMING_REMAINDER_US
                                            .get_or_init(|| Mutex::new(0))
                                            .lock()
                                            .ok();
                                        if let Some(mut rem) = remainder {
                                            let total_us = elapsed_us + *rem;
                                            let elapsed_ms = total_us / 1000;
                                            *rem = total_us % 1000;
                                            if elapsed_ms > 0 {
                                                q.push_back(Instruction::Wait(elapsed_ms, 0));
                                            }
                                        }
                                    }
                                    q.push_back(instr);
                                }
                            }
                        }

                        return Some(event);
                    }

                    // Hotkey detection (only when not recording)
                    if let EventType::KeyPress(key) = &event.event_type {
                        if modifier_bit(key) == 0 {
                            let key_name = format!("{:?}", key);
                            let mods = held_mods.load(Ordering::Relaxed);
                            if let Some(action) = check_hotkey(mods, &key_name) {
                                if let Ok(mut q) = get_hotkey_action_queue().lock() {
                                    q.push_back(action);
                                }
                                return None;
                            }
                        }
                    }

                    Some(event)
                });
                if let Err(e) = result {
                    warn!("rdev grab failed (global hotkeys and recording unavailable): {:?}", e);
                }
            });
    });
}

fn event_to_instruction(event_type: &EventType) -> Option<Instruction> {
    match event_type {
        EventType::KeyPress(key) => Some(Instruction::Token(Token::Key(
            map_rdev_key(*key)?,
            Direction::Press,
        ))),
        EventType::KeyRelease(key) => Some(Instruction::Token(Token::Key(
            map_rdev_key(*key)?,
            Direction::Release,
        ))),
        EventType::ButtonPress(btn) => Some(Instruction::Token(Token::Button(
            map_rdev_button(*btn)?,
            Direction::Press,
        ))),
        EventType::ButtonRelease(btn) => Some(Instruction::Token(Token::Button(
            map_rdev_button(*btn)?,
            Direction::Release,
        ))),
        EventType::Wheel { delta_x, delta_y } => {
            if *delta_y != 0 {
                Some(Instruction::Token(Token::Scroll(
                    *delta_y as i32,
                    Axis::Vertical,
                )))
            } else if *delta_x != 0 {
                Some(Instruction::Token(Token::Scroll(
                    *delta_x as i32,
                    Axis::Horizontal,
                )))
            } else {
                None
            }
        }
        EventType::MouseMove { x, y } => {
            if RECORD_MOUSE_RELATIVE.load(Ordering::Relaxed) {
                let last_pos = LAST_MOUSE_POS.get_or_init(|| Mutex::new(None));
                if let Ok(mut last) = last_pos.lock() {
                    let result = last.map(|(lx, ly)| {
                        let dx = (x - lx).round() as i32;
                        let dy = (y - ly).round() as i32;
                        (dx, dy)
                    });
                    *last = Some((*x, *y));
                    let (dx, dy) = result?;
                    if dx == 0 && dy == 0 {
                        return None;
                    }
                    Some(Instruction::Token(Token::MoveMouse(dx, dy, Coordinate::Rel)))
                } else {
                    None
                }
            } else {
                None
                // TODO fix absolute moving
                /*Some(Instruction::Token(Token::MoveMouse(
                    *x as i32,
                    *y as i32,
                    Coordinate::Abs,
                )))*/
            }
        }
    }
}
