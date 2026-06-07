use crate::hotkey_types::{HotkeyAction, HotkeyBinding};
use crate::input::types::{Axis, Coordinate, Direction, InputToken, MacroKey};
use crate::macros::backend::{self, CaptureDecision, CaptureEvent};
use crate::macros::Instruction;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Instant;
use tracing::warn;

pub(crate) static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) static RECORD_MOUSE_RELATIVE: AtomicBool = AtomicBool::new(false);
static GRAB_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) fn grab_failed() -> bool {
    GRAB_FAILED.load(Ordering::Relaxed)
}

static RECORDING_QUEUE: OnceLock<Mutex<VecDeque<Instruction>>> = OnceLock::new();
static STOP_SIGNAL: OnceLock<Mutex<VecDeque<()>>> = OnceLock::new();
static LAST_EVENT_TIME: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static TIMING_REMAINDER_US: OnceLock<Mutex<u64>> = OnceLock::new();
static LAST_MOUSE_POS: OnceLock<Mutex<Option<(f64, f64)>>> = OnceLock::new();

static HOTKEY_TABLE: OnceLock<RwLock<Vec<HotkeyBinding>>> = OnceLock::new();
static HOTKEY_ACTION_QUEUE: OnceLock<Mutex<VecDeque<HotkeyAction>>> = OnceLock::new();

pub(crate) fn get_last_mouse_pos() -> Option<(f64, f64)> {
    LAST_MOUSE_POS.get_or_init(|| Mutex::new(None)).lock().ok().and_then(|g| *g)
}

pub(crate) fn set_last_mouse_pos(x: f64, y: f64) {
    if let Ok(mut g) = LAST_MOUSE_POS.get_or_init(|| Mutex::new(None)).lock() {
        *g = Some((x, y));
    }
}

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
    #[cfg(windows)]
    {
        crate::macros::backend::windows::signal_hotkey_update(bindings);
        return;
    }
    #[allow(unreachable_code)]
    {
        let table = HOTKEY_TABLE.get_or_init(|| RwLock::new(vec![]));
        if let Ok(mut t) = table.write() {
            *t = bindings;
        }
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
        let held_mods = AtomicU8::new(0);

        backend::start_capture(Box::new(move |event: CaptureEvent| {
            match &event {
                CaptureEvent::KeyPress(key) => {
                    let bit = key.modifier_bit();
                    if bit != 0 {
                        held_mods.fetch_or(bit, Ordering::Relaxed);
                    }
                }
                CaptureEvent::KeyRelease(key) => {
                    let bit = key.modifier_bit();
                    if bit != 0 {
                        held_mods.fetch_and(!bit, Ordering::Relaxed);
                    }
                }
                _ => {}
            }

            if RECORDING_ACTIVE.load(Ordering::Relaxed) {
                // Escape stops recording
                if matches!(&event, CaptureEvent::KeyPress(MacroKey::Escape)) {
                    RECORDING_ACTIVE.store(false, Ordering::Relaxed);
                    if let Ok(mut q) = get_stop_signal().lock() {
                        q.push_back(());
                    }
                    return CaptureDecision::Suppress;
                }

                let last_time = LAST_EVENT_TIME.get_or_init(|| Mutex::new(None));
                let now = Instant::now();
                let instr = capture_event_to_instruction(&event);
                if let Some(instr) = instr {
                    if let Ok(mut last) = last_time.lock() {
                        let prev = *last;
                        *last = Some(now);
                        if let Ok(mut q) = get_recording_queue().lock() {
                            if let Some(prev_time) = prev {
                                let elapsed_us = now.duration_since(prev_time).as_micros() as u64;
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

                return CaptureDecision::Passthrough;
            }

            // Track real cursor position for relative-move playback.
            match &event {
                CaptureEvent::MouseMoveRel(dx, dy) => {
                    if let Some((lx, ly)) = get_last_mouse_pos() {
                        set_last_mouse_pos(lx + *dx as f64, ly + *dy as f64);
                    }
                }
                CaptureEvent::MouseMoveAbs(x, y) => {
                    set_last_mouse_pos(*x, *y);
                }
                _ => {}
            }

            // Hotkey detection (only when not recording).
            // On Windows, RegisterHotKey/WM_HOTKEY handles this instead.
            #[cfg(not(windows))]
            if let CaptureEvent::KeyPress(key) = &event {
                if !key.is_modifier() {
                    if let Some(name) = key.hotkey_name() {
                        let mods = held_mods.load(Ordering::Relaxed);
                        if let Some(action) = check_hotkey(mods, &name) {
                            if let Ok(mut q) = get_hotkey_action_queue().lock() {
                                q.push_back(action);
                            }
                            return CaptureDecision::Suppress;
                        }
                    }
                }
            }

            CaptureDecision::Passthrough
        }));
    });
}

fn capture_event_to_instruction(event: &CaptureEvent) -> Option<Instruction> {
    Some(match event {
        CaptureEvent::KeyPress(key) => {
            Instruction::Token(InputToken::Key(key.clone(), Direction::Press))
        }
        CaptureEvent::KeyRelease(key) => {
            Instruction::Token(InputToken::Key(key.clone(), Direction::Release))
        }
        CaptureEvent::ButtonPress(btn) => {
            Instruction::Token(InputToken::Button(btn.clone(), Direction::Press))
        }
        CaptureEvent::ButtonRelease(btn) => {
            Instruction::Token(InputToken::Button(btn.clone(), Direction::Release))
        }
        CaptureEvent::Scroll(h, v) => {
            if *v != 0 {
                Instruction::Token(InputToken::Scroll(*v, Axis::Vertical))
            } else if *h != 0 {
                Instruction::Token(InputToken::Scroll(*h, Axis::Horizontal))
            } else {
                return None;
            }
        }
        CaptureEvent::MouseMoveRel(dx, dy) => {
            if !RECORD_MOUSE_RELATIVE.load(Ordering::Relaxed) {
                return None;
            }
            if *dx == 0 && *dy == 0 {
                return None;
            }
            Instruction::Token(InputToken::MoveMouse(*dx, *dy, Coordinate::Rel))
        }
        CaptureEvent::MouseMoveAbs(x, y) => {
            Instruction::Token(InputToken::MoveMouse(*x as i32, *y as i32, Coordinate::Abs))
        }
    })
}
