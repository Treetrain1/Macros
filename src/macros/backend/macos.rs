use std::sync::OnceLock;

use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CGMouseButton, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use tracing::warn;

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent, CaptureTimestamp, InputBackend};

// ── Key mapping ───────────────────────────────────────────────────────────────

fn macro_key_to_cgkeycode(key: &MacroKey) -> Option<(u16, bool)> {
    // CGKeyCode values from HIToolbox/Events.h
    Some(match key {
        MacroKey::Return => (36, false),
        MacroKey::Backspace => (51, false),
        MacroKey::Tab => (48, false),
        MacroKey::Space => (49, false),
        MacroKey::Escape => (53, false),
        MacroKey::Delete => (117, false),
        MacroKey::Insert => return None, // no Insert on macOS
        MacroKey::Home => (115, false),
        MacroKey::End => (119, false),
        MacroKey::PageUp => (116, false),
        MacroKey::PageDown => (121, false),
        MacroKey::UpArrow => (126, false),
        MacroKey::DownArrow => (125, false),
        MacroKey::LeftArrow => (123, false),
        MacroKey::RightArrow => (124, false),
        MacroKey::Shift | MacroKey::LShift => (56, false),
        MacroKey::RShift => (60, false),
        MacroKey::Control | MacroKey::LControl => (59, false),
        MacroKey::RControl => (62, false),
        MacroKey::Alt | MacroKey::Option => (58, false),
        MacroKey::AltGr => (61, false),
        MacroKey::Meta => (55, false),
        MacroKey::CapsLock => (57, false),
        MacroKey::F1 => (122, false),
        MacroKey::F2 => (120, false),
        MacroKey::F3 => (99, false),
        MacroKey::F4 => (118, false),
        MacroKey::F5 => (96, false),
        MacroKey::F6 => (97, false),
        MacroKey::F7 => (98, false),
        MacroKey::F8 => (100, false),
        MacroKey::F9 => (101, false),
        MacroKey::F10 => (109, false),
        MacroKey::F11 => (103, false),
        MacroKey::F12 => (111, false),
        MacroKey::Unicode(c) => char_to_cgkeycode(*c)?,
        MacroKey::Other(n) => (*n as u16, false),
        _ => return None,
    })
}

fn char_to_cgkeycode(c: char) -> Option<(u16, bool)> {
    // Standard US keyboard layout CGKeyCodes.
    Some(match c {
        'a' | 'A' => (0, c.is_uppercase()),
        'b' | 'B' => (11, c.is_uppercase()),
        'c' | 'C' => (8, c.is_uppercase()),
        'd' | 'D' => (2, c.is_uppercase()),
        'e' | 'E' => (14, c.is_uppercase()),
        'f' | 'F' => (3, c.is_uppercase()),
        'g' | 'G' => (5, c.is_uppercase()),
        'h' | 'H' => (4, c.is_uppercase()),
        'i' | 'I' => (34, c.is_uppercase()),
        'j' | 'J' => (38, c.is_uppercase()),
        'k' | 'K' => (40, c.is_uppercase()),
        'l' | 'L' => (37, c.is_uppercase()),
        'm' | 'M' => (46, c.is_uppercase()),
        'n' | 'N' => (45, c.is_uppercase()),
        'o' | 'O' => (31, c.is_uppercase()),
        'p' | 'P' => (35, c.is_uppercase()),
        'q' | 'Q' => (12, c.is_uppercase()),
        'r' | 'R' => (15, c.is_uppercase()),
        's' | 'S' => (1, c.is_uppercase()),
        't' | 'T' => (17, c.is_uppercase()),
        'u' | 'U' => (32, c.is_uppercase()),
        'v' | 'V' => (9, c.is_uppercase()),
        'w' | 'W' => (13, c.is_uppercase()),
        'x' | 'X' => (7, c.is_uppercase()),
        'y' | 'Y' => (16, c.is_uppercase()),
        'z' | 'Z' => (6, c.is_uppercase()),
        '0' | ')' => (29, c == ')'),
        '1' | '!' => (18, c == '!'),
        '2' | '@' => (19, c == '@'),
        '3' | '#' => (20, c == '#'),
        '4' | '$' => (21, c == '$'),
        '5' | '%' => (23, c == '%'),
        '6' | '^' => (22, c == '^'),
        '7' | '&' => (26, c == '&'),
        '8' | '*' => (28, c == '*'),
        '9' | '(' => (25, c == '('),
        '-' | '_' => (27, c == '_'),
        '=' | '+' => (24, c == '+'),
        '[' | '{' => (33, c == '{'),
        ']' | '}' => (30, c == '}'),
        '\\' | '|' => (42, c == '|'),
        ';' | ':' => (41, c == ':'),
        '\'' | '"' => (39, c == '"'),
        '`' | '~' => (50, c == '~'),
        ',' | '<' => (43, c == '<'),
        '.' | '>' => (47, c == '>'),
        '/' | '?' => (44, c == '?'),
        ' ' => (49, false),
        '\n' => (36, false),
        '\t' => (48, false),
        _ => return None,
    })
}

// ── InputBackend impl ─────────────────────────────────────────────────────────

pub struct MacosBackend {
    source: CGEventSource,
}

impl MacosBackend {
    pub fn new() -> Self {
        Self {
            source: CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .expect("CGEventSource"),
        }
    }

    fn post(&self, event: CGEvent) {
        event.post(CGEventTapLocation::HID);
    }

    fn emit_key(&self, keycode: u16, down: bool, needs_shift: bool) {
        if needs_shift {
            let ev = CGEvent::new_keyboard_event(self.source.clone(), 56, true).unwrap();
            self.post(ev);
        }
        let ev = CGEvent::new_keyboard_event(self.source.clone(), keycode, down).unwrap();
        self.post(ev);
        if needs_shift && !down {
            let ev = CGEvent::new_keyboard_event(self.source.clone(), 56, false).unwrap();
            self.post(ev);
        }
    }
}

impl InputBackend for MacosBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        let (code, needs_shift) =
            macro_key_to_cgkeycode(&key).ok_or_else(|| format!("no CGKeyCode for {:?}", key))?;
        match dir {
            Direction::Press => {
                if needs_shift { self.emit_key(56, true, false); }
                self.emit_key(code, true, false);
            }
            Direction::Release => {
                self.emit_key(code, false, false);
                if needs_shift { self.emit_key(56, false, false); }
            }
            Direction::Click => {
                if needs_shift { self.emit_key(56, true, false); }
                self.emit_key(code, true, false);
                self.emit_key(code, false, false);
                if needs_shift { self.emit_key(56, false, false); }
            }
        }
        Ok(())
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        match dir {
            Direction::Press => self.emit_key(keycode, true, false),
            Direction::Release => self.emit_key(keycode, false, false),
            Direction::Click => {
                self.emit_key(keycode, true, false);
                self.emit_key(keycode, false, false);
            }
        }
        Ok(())
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        let cur = self.cursor_pos().unwrap_or((0, 0));
        let pos = CGPoint::new(cur.0 as f64, cur.1 as f64);

        let (down_ty, up_ty, mouse_btn) = match button {
            MacroButton::ScrollUp => {
                let ev = CGEvent::new_scroll_event(self.source.clone(), core_graphics::event::ScrollEventUnit::Line, 1, 1, 0, 0).ok();
                if let Some(ev) = ev { self.post(ev); }
                return Ok(());
            }
            MacroButton::ScrollDown => {
                let ev = CGEvent::new_scroll_event(self.source.clone(), core_graphics::event::ScrollEventUnit::Line, 1, -1, 0, 0).ok();
                if let Some(ev) = ev { self.post(ev); }
                return Ok(());
            }
            MacroButton::ScrollLeft => {
                let ev = CGEvent::new_scroll_event(self.source.clone(), core_graphics::event::ScrollEventUnit::Line, 2, 0, -1, 0).ok();
                if let Some(ev) = ev { self.post(ev); }
                return Ok(());
            }
            MacroButton::ScrollRight => {
                let ev = CGEvent::new_scroll_event(self.source.clone(), core_graphics::event::ScrollEventUnit::Line, 2, 0, 1, 0).ok();
                if let Some(ev) = ev { self.post(ev); }
                return Ok(());
            }
            MacroButton::Left => (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left),
            MacroButton::Right => (CGEventType::RightMouseDown, CGEventType::RightMouseUp, CGMouseButton::Right),
            MacroButton::Middle | MacroButton::Other(_) => (CGEventType::OtherMouseDown, CGEventType::OtherMouseUp, CGMouseButton::Center),
            MacroButton::Back | MacroButton::Forward => return Ok(()),
        };

        match dir {
            Direction::Press | Direction::Click => {
                let ev = CGEvent::new_mouse_event(self.source.clone(), down_ty, pos, mouse_btn)
                    .map_err(|_| format!("CGEvent mouse down failed"))?;
                self.post(ev);
                if matches!(dir, Direction::Click) {
                    let ev = CGEvent::new_mouse_event(self.source.clone(), up_ty, pos, mouse_btn)
                        .map_err(|_| format!("CGEvent mouse up failed"))?;
                    self.post(ev);
                }
            }
            Direction::Release => {
                let ev = CGEvent::new_mouse_event(self.source.clone(), up_ty, pos, mouse_btn)
                    .map_err(|_| format!("CGEvent mouse up failed"))?;
                self.post(ev);
            }
        }
        Ok(())
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        let cur = self.cursor_pos().unwrap_or((0, 0));
        let new_x = cur.0 + dx;
        let new_y = cur.1 + dy;
        let pos = CGPoint::new(new_x as f64, new_y as f64);
        let ev = CGEvent::new_mouse_event(
            self.source.clone(),
            CGEventType::MouseMoved,
            pos,
            CGMouseButton::Left,
        )
        .map_err(|_| "CGEvent mouse move failed".to_string())?;
        ev.set_double_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as f64);
        ev.set_double_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as f64);
        self.post(ev);
        Ok(())
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        let cur = self.cursor_pos().unwrap_or((0, 0));
        self.move_mouse_rel(x - cur.0, y - cur.1)
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        let (v, h) = match axis {
            Axis::Vertical => (amount, 0),
            Axis::Horizontal => (0, amount),
        };
        let ev = CGEvent::new_scroll_event(
            self.source.clone(),
            core_graphics::event::ScrollEventUnit::Line,
            2,
            v,
            h,
            0,
        )
        .map_err(|_| "CGEvent scroll failed".to_string())?;
        self.post(ev);
        Ok(())
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        for c in s.chars() {
            let _ = self.key(MacroKey::Unicode(c), Direction::Click);
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        let loc = CGEvent::new(self.source.clone()).ok()?.location();
        Some((loc.x as i32, loc.y as i32))
    }
}

// ── Capture ───────────────────────────────────────────────────────────────────

pub(super) fn start_capture_thread(
    callback: Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static>,
) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::Builder::new()
            .name("cgeventtap".into())
            .spawn(move || {
                use std::sync::Mutex;
                let callback = Mutex::new(callback);

                let tap = CGEventTap::new(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    vec![
                        CGEventType::KeyDown,
                        CGEventType::KeyUp,
                        CGEventType::MouseMoved,
                        CGEventType::LeftMouseDown,
                        CGEventType::LeftMouseUp,
                        CGEventType::RightMouseDown,
                        CGEventType::RightMouseUp,
                        CGEventType::OtherMouseDown,
                        CGEventType::OtherMouseUp,
                        CGEventType::ScrollWheel,
                    ],
                    |_proxy, ev_type, event| {
                        let capture_ev: Option<CaptureEvent> = match ev_type {
                            CGEventType::KeyDown => {
                                let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                                cg_keycode_to_macro_key(keycode).map(CaptureEvent::KeyPress)
                            }
                            CGEventType::KeyUp => {
                                let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                                cg_keycode_to_macro_key(keycode).map(CaptureEvent::KeyRelease)
                            }
                            CGEventType::MouseMoved => {
                                let dx = event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_X) as i32;
                                let dy = event.get_double_value_field(EventField::MOUSE_EVENT_DELTA_Y) as i32;
                                Some(CaptureEvent::MouseMoveRel(dx, dy))
                            }
                            CGEventType::LeftMouseDown => Some(CaptureEvent::ButtonPress(MacroButton::Left)),
                            CGEventType::LeftMouseUp => Some(CaptureEvent::ButtonRelease(MacroButton::Left)),
                            CGEventType::RightMouseDown => Some(CaptureEvent::ButtonPress(MacroButton::Right)),
                            CGEventType::RightMouseUp => Some(CaptureEvent::ButtonRelease(MacroButton::Right)),
                            CGEventType::ScrollWheel => {
                                let v = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1) as i32;
                                let h = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2) as i32;
                                Some(CaptureEvent::Scroll(h, v))
                            }
                            _ => None,
                        };

                        if let Some(ev) = capture_ev {
                            if let Ok(mut cb) = callback.lock() {
                                if matches!(cb(ev, CaptureTimestamp::Now), CaptureDecision::Suppress) {
                                    return None;
                                }
                            }
                        }
                        Some(event.clone())
                    },
                );

                match tap {
                    Ok(tap) => {
                        let source = tap.mach_port.create_runloop_source(0).unwrap();
                        CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopDefaultMode });
                        tap.enable();
                        CFRunLoop::run_current();
                    }
                    Err(_) => {
                        warn!("CGEventTap creation failed (check Accessibility permissions)");
                    }
                }
            })
            .ok();
    });
}

fn cg_keycode_to_macro_key(code: u16) -> Option<MacroKey> {
    Some(match code {
        36 => MacroKey::Return,
        51 => MacroKey::Backspace,
        48 => MacroKey::Tab,
        49 => MacroKey::Space,
        53 => MacroKey::Escape,
        117 => MacroKey::Delete,
        115 => MacroKey::Home,
        119 => MacroKey::End,
        116 => MacroKey::PageUp,
        121 => MacroKey::PageDown,
        126 => MacroKey::UpArrow,
        125 => MacroKey::DownArrow,
        123 => MacroKey::LeftArrow,
        124 => MacroKey::RightArrow,
        56 => MacroKey::LShift,
        60 => MacroKey::RShift,
        59 => MacroKey::LControl,
        62 => MacroKey::RControl,
        58 => MacroKey::Alt,
        61 => MacroKey::AltGr,
        55 => MacroKey::Meta,
        57 => MacroKey::CapsLock,
        122 => MacroKey::F1,
        120 => MacroKey::F2,
        99 => MacroKey::F3,
        118 => MacroKey::F4,
        96 => MacroKey::F5,
        97 => MacroKey::F6,
        98 => MacroKey::F7,
        100 => MacroKey::F8,
        101 => MacroKey::F9,
        109 => MacroKey::F10,
        103 => MacroKey::F11,
        111 => MacroKey::F12,
        0 => MacroKey::Unicode('a'),
        11 => MacroKey::Unicode('b'),
        8 => MacroKey::Unicode('c'),
        2 => MacroKey::Unicode('d'),
        14 => MacroKey::Unicode('e'),
        3 => MacroKey::Unicode('f'),
        5 => MacroKey::Unicode('g'),
        4 => MacroKey::Unicode('h'),
        34 => MacroKey::Unicode('i'),
        38 => MacroKey::Unicode('j'),
        40 => MacroKey::Unicode('k'),
        37 => MacroKey::Unicode('l'),
        46 => MacroKey::Unicode('m'),
        45 => MacroKey::Unicode('n'),
        31 => MacroKey::Unicode('o'),
        35 => MacroKey::Unicode('p'),
        12 => MacroKey::Unicode('q'),
        15 => MacroKey::Unicode('r'),
        1 => MacroKey::Unicode('s'),
        17 => MacroKey::Unicode('t'),
        32 => MacroKey::Unicode('u'),
        9 => MacroKey::Unicode('v'),
        13 => MacroKey::Unicode('w'),
        7 => MacroKey::Unicode('x'),
        16 => MacroKey::Unicode('y'),
        6 => MacroKey::Unicode('z'),
        18 => MacroKey::Unicode('1'),
        19 => MacroKey::Unicode('2'),
        20 => MacroKey::Unicode('3'),
        21 => MacroKey::Unicode('4'),
        23 => MacroKey::Unicode('5'),
        22 => MacroKey::Unicode('6'),
        26 => MacroKey::Unicode('7'),
        28 => MacroKey::Unicode('8'),
        25 => MacroKey::Unicode('9'),
        29 => MacroKey::Unicode('0'),
        27 => MacroKey::Unicode('-'),
        24 => MacroKey::Unicode('='),
        33 => MacroKey::Unicode('['),
        30 => MacroKey::Unicode(']'),
        42 => MacroKey::Unicode('\\'),
        41 => MacroKey::Unicode(';'),
        39 => MacroKey::Unicode('\''),
        50 => MacroKey::Unicode('`'),
        43 => MacroKey::Unicode(','),
        47 => MacroKey::Unicode('.'),
        44 => MacroKey::Unicode('/'),
        _ => MacroKey::Other(code as u32),
    })
}
