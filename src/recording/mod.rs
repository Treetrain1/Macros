use crate::hotkey_types::{HotkeyAction, HotkeyBinding};
use crate::input::types::{Axis, Coordinate, Direction, InputToken, MacroKey};
use crate::macros::backend::{self, CaptureDecision, CaptureEvent, CaptureTimestamp};
use crate::macros::Instruction;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime};
use tracing::warn;

pub(crate) static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) static RECORD_MOUSE_RELATIVE: AtomicBool = AtomicBool::new(false);
static GRAB_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) fn grab_failed() -> bool {
    GRAB_FAILED.load(Ordering::Relaxed)
}

static RECORDING_QUEUE: OnceLock<Mutex<VecDeque<Instruction>>> = OnceLock::new();
static STOP_SIGNAL: OnceLock<Mutex<VecDeque<()>>> = OnceLock::new();
// Anchors for whichever `CaptureTimestamp` variant a session's backend produces
// (Linux evdev always reports `Hardware`; Windows/macOS always report `Now`),
// so timing is measured against the backend's own clock instead of the time
// the event happens to reach this callback.
static BASELINE_NOW: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static BASELINE_HW: OnceLock<Mutex<Option<SystemTime>>> = OnceLock::new();
static LAST_ELAPSED: OnceLock<Mutex<Option<Duration>>> = OnceLock::new();
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
    if let Ok(mut t) = BASELINE_NOW.get_or_init(|| Mutex::new(None)).lock() {
        *t = None;
    }
    if let Ok(mut t) = BASELINE_HW.get_or_init(|| Mutex::new(None)).lock() {
        *t = None;
    }
    if let Ok(mut t) = LAST_ELAPSED.get_or_init(|| Mutex::new(None)).lock() {
        *t = None;
    }
    if let Ok(mut p) = LAST_MOUSE_POS.get_or_init(|| Mutex::new(None)).lock() {
        *p = None;
    }
}

/// Elapsed time since this recording session's first event, measured on
/// whichever clock the backend reported `ts` against. Negative deltas
/// (e.g. a `SystemTime` clock step, or two evdev devices' messages arriving
/// at the dispatch channel out of hardware-timestamp order) clamp to zero
/// rather than corrupting the recording.
fn elapsed_since_session_start(ts: CaptureTimestamp) -> Duration {
    match ts {
        CaptureTimestamp::Now => {
            let now = Instant::now();
            let baseline = BASELINE_NOW.get_or_init(|| Mutex::new(None));
            let mut baseline = match baseline.lock() {
                Ok(g) => g,
                Err(_) => return Duration::ZERO,
            };
            let start = *baseline.get_or_insert(now);
            now.saturating_duration_since(start)
        }
        CaptureTimestamp::Hardware(now) => {
            let baseline = BASELINE_HW.get_or_init(|| Mutex::new(None));
            let mut baseline = match baseline.lock() {
                Ok(g) => g,
                Err(_) => return Duration::ZERO,
            };
            let start = *baseline.get_or_insert(now);
            now.duration_since(start).unwrap_or(Duration::ZERO)
        }
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

        backend::start_capture(Box::new(move |event: CaptureEvent, ts: CaptureTimestamp| {
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

                let elapsed = elapsed_since_session_start(ts);
                let instr = capture_event_to_instruction(&event);
                if let Some(instr) = instr {
                    let last_elapsed = LAST_ELAPSED.get_or_init(|| Mutex::new(None));
                    if let Ok(mut last) = last_elapsed.lock() {
                        let prev = *last;
                        *last = Some(elapsed);
                        if let Ok(mut q) = get_recording_queue().lock() {
                            if let Some(prev_elapsed) = prev {
                                let elapsed_ms = elapsed.saturating_sub(prev_elapsed).as_secs_f64() * 1000.0;
                                if elapsed_ms > 0.0 {
                                    q.push_back(Instruction::Wait(elapsed_ms, 0.0));
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
