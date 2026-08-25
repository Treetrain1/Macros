//! `InputBackend` that emits through the X Test extension (via `enigo`'s
//! default `x11rb` backend) instead of a `uinput` virtual device.
//!
//! `evdev::EvdevBackend`'s emission writes to `/dev/uinput`, which the
//! kernel turns into a brand-new input device that then has to be picked up
//! by `libinput`, routed through the compositor's input dispatch, and (for
//! an XWayland client, which is what Wine/Proton games are in practice)
//! bridged into X11 — several hops a real X11 client's input never takes.
//! XTest instead talks directly to the X server (`XTestFakeKeyEvent`/
//! `XTestFakeButtonEvent`/`XTestFakeMotionEvent`), landing the event exactly
//! where a real X11 input event would, skipping the kernel-evdev/libinput/
//! compositor hops entirely. That gap was consistently on the order of a
//! single 240 TPS tick (~4ms) end to end under Wine — small, but exactly
//! frame/tick-precise enough to be the difference between clearing an
//! obstacle and not.
//!
//! Capture is unaffected by any of this and stays on evdev (reading real
//! hardware devices directly) — this module is emission-only.

use enigo::{Enigo, Keyboard, Mouse, Settings};

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::InputBackend;

/// Maps this app's `MacroKey` to an `enigo::Key`, the same way
/// `backend::windows::macro_key_to_enigo_key` does — except the two keys
/// enigo has no named cross-platform variant for (`AltGr`, `Select`) need
/// different raw codes here: `enigo::Key::Other(n)` is a Windows VK code on
/// the Windows backend but an X11 *keysym* on this one, so the Windows
/// backend's `Other(VK_RMENU as u32)`/`Other(0x29)` don't carry over as-is.
fn macro_key_to_enigo_key(key: &MacroKey) -> enigo::Key {
    use enigo::Key;
    match key {
        MacroKey::Return => Key::Return,
        MacroKey::Backspace => Key::Backspace,
        MacroKey::Tab => Key::Tab,
        MacroKey::Space => Key::Space,
        MacroKey::Escape => Key::Escape,
        MacroKey::Delete => Key::Delete,
        MacroKey::Insert => Key::Insert,
        MacroKey::Home => Key::Home,
        MacroKey::End => Key::End,
        MacroKey::PageUp => Key::PageUp,
        MacroKey::PageDown => Key::PageDown,
        MacroKey::UpArrow => Key::UpArrow,
        MacroKey::DownArrow => Key::DownArrow,
        MacroKey::LeftArrow => Key::LeftArrow,
        MacroKey::RightArrow => Key::RightArrow,
        MacroKey::Shift => Key::Shift,
        MacroKey::LShift => Key::LShift,
        MacroKey::RShift => Key::RShift,
        MacroKey::Control => Key::Control,
        MacroKey::LControl => Key::LControl,
        MacroKey::RControl => Key::RControl,
        MacroKey::Alt | MacroKey::Option => Key::Alt,
        // X11 keysym for ISO_Level3_Shift (the standard "AltGr" keysym) -
        // see the module doc comment for why this differs from the
        // Windows backend's VK-code-based `Other(...)`.
        MacroKey::AltGr => Key::Other(0xfe03),
        MacroKey::Meta | MacroKey::LMenu => Key::Meta,
        MacroKey::CapsLock => Key::CapsLock,
        MacroKey::NumLock => Key::Numlock,
        MacroKey::ScrollLock => Key::ScrollLock,
        MacroKey::Pause => Key::Pause,
        MacroKey::PrintScr => Key::PrintScr,
        MacroKey::F1 => Key::F1,
        MacroKey::F2 => Key::F2,
        MacroKey::F3 => Key::F3,
        MacroKey::F4 => Key::F4,
        MacroKey::F5 => Key::F5,
        MacroKey::F6 => Key::F6,
        MacroKey::F7 => Key::F7,
        MacroKey::F8 => Key::F8,
        MacroKey::F9 => Key::F9,
        MacroKey::F10 => Key::F10,
        MacroKey::F11 => Key::F11,
        MacroKey::F12 => Key::F12,
        MacroKey::F13 => Key::F13,
        MacroKey::F14 => Key::F14,
        MacroKey::F15 => Key::F15,
        MacroKey::F16 => Key::F16,
        MacroKey::F17 => Key::F17,
        MacroKey::F18 => Key::F18,
        MacroKey::F19 => Key::F19,
        MacroKey::F20 => Key::F20,
        MacroKey::F21 => Key::F21,
        MacroKey::F22 => Key::F22,
        MacroKey::F23 => Key::F23,
        MacroKey::F24 => Key::F24,
        MacroKey::Numpad0 => Key::Numpad0,
        MacroKey::Numpad1 => Key::Numpad1,
        MacroKey::Numpad2 => Key::Numpad2,
        MacroKey::Numpad3 => Key::Numpad3,
        MacroKey::Numpad4 => Key::Numpad4,
        MacroKey::Numpad5 => Key::Numpad5,
        MacroKey::Numpad6 => Key::Numpad6,
        MacroKey::Numpad7 => Key::Numpad7,
        MacroKey::Numpad8 => Key::Numpad8,
        MacroKey::Numpad9 => Key::Numpad9,
        MacroKey::Add => Key::Add,
        MacroKey::Subtract => Key::Subtract,
        MacroKey::Multiply => Key::Multiply,
        MacroKey::Divide => Key::Divide,
        MacroKey::Decimal => Key::Decimal,
        MacroKey::VolumeDown => Key::VolumeDown,
        MacroKey::VolumeMute => Key::VolumeMute,
        MacroKey::VolumeUp => Key::VolumeUp,
        // enigo has a real cross-platform `Select` variant (unlike AltGr),
        // so no raw code needed here.
        MacroKey::Select => Key::Select,
        MacroKey::Unicode(c) => Key::Unicode(*c),
        MacroKey::Other(n) => Key::Other(*n),
    }
}

fn to_enigo_dir(dir: Direction) -> enigo::Direction {
    match dir {
        Direction::Press => enigo::Direction::Press,
        Direction::Release => enigo::Direction::Release,
        Direction::Click => enigo::Direction::Click,
    }
}

pub struct XTestBackend {
    enigo: Enigo,
}

impl XTestBackend {
    pub fn new() -> Result<Self, String> {
        let settings = Settings {
            // No auto-release-on-drop: the runner's own strand cleanup
            // already releases held keys/buttons explicitly (see
            // `run_strand`), matching the Windows backend's setting for
            // the same reason.
            release_keys_when_dropped: false,
            ..Default::default()
        };
        let enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
        Ok(Self { enigo })
    }
}

impl InputBackend for XTestBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        let key = macro_key_to_enigo_key(&key);
        self.enigo.key(key, to_enigo_dir(dir)).map_err(|e| e.to_string())
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        self.enigo
            .key(enigo::Key::Other(keycode as u32), to_enigo_dir(dir))
            .map_err(|e| e.to_string())
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        use enigo::{Axis as EAxis, Button as EButton};
        let dir = to_enigo_dir(dir);
        match button {
            MacroButton::ScrollUp => self.enigo.scroll(-1, EAxis::Vertical),
            MacroButton::ScrollDown => self.enigo.scroll(1, EAxis::Vertical),
            MacroButton::ScrollLeft => self.enigo.scroll(-1, EAxis::Horizontal),
            MacroButton::ScrollRight => self.enigo.scroll(1, EAxis::Horizontal),
            MacroButton::Left => self.enigo.button(EButton::Left, dir),
            MacroButton::Right => self.enigo.button(EButton::Right, dir),
            MacroButton::Middle => self.enigo.button(EButton::Middle, dir),
            MacroButton::Back => self.enigo.button(EButton::Back, dir),
            MacroButton::Forward => self.enigo.button(EButton::Forward, dir),
            MacroButton::Other(_) => Ok(()),
        }
        .map_err(|e| e.to_string())
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.enigo
            .move_mouse(dx, dy, enigo::Coordinate::Rel)
            .map_err(|e| e.to_string())
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.enigo
            .move_mouse(x, y, enigo::Coordinate::Abs)
            .map_err(|e| e.to_string())
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        let (length, axis) = match axis {
            // Matches the Windows backend's sign convention (this trait's
            // "positive = up") against enigo's own inverted-vertical
            // internal convention.
            Axis::Vertical => (-amount, enigo::Axis::Vertical),
            Axis::Horizontal => (amount, enigo::Axis::Horizontal),
        };
        self.enigo.scroll(length, axis).map_err(|e| e.to_string())
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        self.enigo.text(s).map_err(|e| e.to_string())
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        self.enigo.location().ok()
    }
}
