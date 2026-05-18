#![cfg(target_os = "linux")]

use crate::input::rdev_mapping::{map_rdev_button, map_rdev_key};
use crate::macros::Instruction;
use enigo::agent::Token;
use enigo::{Axis, Coordinate, Direction};
use rdev::EventType;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub(crate) static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) static RECORD_MOUSE_RELATIVE: AtomicBool = AtomicBool::new(false);

static RECORDING_QUEUE: OnceLock<Mutex<VecDeque<Instruction>>> = OnceLock::new();
static STOP_SIGNAL: OnceLock<Mutex<VecDeque<()>>> = OnceLock::new();
static LAST_EVENT_TIME: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static TIMING_REMAINDER_US: OnceLock<Mutex<u64>> = OnceLock::new();
static LAST_MOUSE_POS: OnceLock<Mutex<Option<(f64, f64)>>> = OnceLock::new();

pub(crate) fn get_recording_queue() -> &'static Mutex<VecDeque<Instruction>> {
    RECORDING_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(crate) fn get_stop_signal() -> &'static Mutex<VecDeque<()>> {
    STOP_SIGNAL.get_or_init(|| Mutex::new(VecDeque::new()))
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

pub(crate) fn start_grab_thread() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("rdev-grab".into())
            .spawn(|| {
                let _ = rdev::grab(|event| {
                    if !RECORDING_ACTIVE.load(Ordering::Relaxed) {
                        return Some(event);
                    }

                    if matches!(event.event_type, EventType::KeyPress(rdev::Key::Escape)) {
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

                    Some(event)
                });
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
                Some(Instruction::Token(Token::MoveMouse(*x as i32, *y as i32, Coordinate::Abs)))
            }
        }
    }
}
