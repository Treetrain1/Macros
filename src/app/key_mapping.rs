use crate::hotkey_types::{MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
use cosmic::iced::keyboard;
use cosmic::iced::keyboard::key::Named;
use enigo::Key;

pub(crate) fn map_iced_key_to_enigo_key(key: keyboard::Key<&str>) -> Option<Key> {
    match key {
        keyboard::Key::Character(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            if chars.next().is_none() {
                Some(Key::Unicode(c))
            } else {
                None
            }
        }
        keyboard::Key::Named(named) => match named {
            Named::Shift => Some(Key::Shift),
            Named::Control => Some(Key::Control),
            Named::Alt => Some(Key::Alt),
            Named::Super => Some(Key::Meta),
            Named::Meta => Some(Key::Meta),
            Named::Enter => Some(Key::Return),
            Named::Tab => Some(Key::Tab),
            Named::Escape => Some(Key::Escape),
            Named::Backspace => Some(Key::Backspace),
            Named::ArrowLeft => Some(Key::LeftArrow),
            Named::ArrowRight => Some(Key::RightArrow),
            Named::ArrowUp => Some(Key::UpArrow),
            Named::ArrowDown => Some(Key::DownArrow),
            Named::Delete => Some(Key::Delete),
            Named::Insert => Some(Key::Insert),
            Named::Home => Some(Key::Home),
            Named::End => Some(Key::End),
            Named::PageUp => Some(Key::PageUp),
            Named::PageDown => Some(Key::PageDown),
            Named::CapsLock => Some(Key::CapsLock),
            Named::NumLock => Some(Key::Numlock),
            #[cfg(all(unix, not(target_os = "macos")))]
            Named::ScrollLock => Some(Key::ScrollLock),
            Named::F1 => Some(Key::F1),
            Named::F2 => Some(Key::F2),
            Named::F3 => Some(Key::F3),
            Named::F4 => Some(Key::F4),
            Named::F5 => Some(Key::F5),
            Named::F6 => Some(Key::F6),
            Named::F7 => Some(Key::F7),
            Named::F8 => Some(Key::F8),
            Named::F9 => Some(Key::F9),
            Named::F10 => Some(Key::F10),
            Named::F11 => Some(Key::F11),
            Named::F12 => Some(Key::F12),
            Named::F13 => Some(Key::F13),
            Named::F14 => Some(Key::F14),
            Named::F15 => Some(Key::F15),
            Named::F16 => Some(Key::F16),
            Named::F17 => Some(Key::F17),
            Named::F18 => Some(Key::F18),
            Named::F19 => Some(Key::F19),
            Named::F20 => Some(Key::F20),
            Named::F21 => Some(Key::F21),
            Named::F22 => Some(Key::F22),
            Named::F23 => Some(Key::F23),
            Named::F24 => Some(Key::F24),
            _ => None,
        },
        keyboard::Key::Unidentified => None,
    }
}

pub(crate) fn map_iced_key_code_to_enigo_key(code: keyboard::key::Code) -> Option<Key> {
    match code {
        keyboard::key::Code::Space => Some(Key::Space),
        keyboard::key::Code::ShiftLeft => Some(Key::LShift),
        keyboard::key::Code::ShiftRight => Some(Key::RShift),
        keyboard::key::Code::ControlLeft => Some(Key::LControl),
        keyboard::key::Code::ControlRight => Some(Key::RControl),
        keyboard::key::Code::AltLeft => Some(Key::Option),
        keyboard::key::Code::AltRight => Some(Key::Alt),
        _ => None,
    }
}

pub(crate) fn map_iced_physical_key_to_enigo_key(physical_key: keyboard::key::Physical) -> Option<Key> {
    match physical_key {
        keyboard::key::Physical::Code(code) => map_iced_key_code_to_enigo_key(code),
        _ => None,
    }
}

pub(crate) fn mods_to_u8(mods: &keyboard::Modifiers) -> u8 {
    let mut result = 0u8;
    if mods.control() {
        result |= MOD_CTRL;
    }
    if mods.shift() {
        result |= MOD_SHIFT;
    }
    if mods.alt() {
        result |= MOD_ALT;
    }
    if mods.logo() {
        result |= MOD_META;
    }
    result
}

pub(crate) fn is_modifier_code(physical_key: &keyboard::key::Physical) -> bool {
    use keyboard::key::{Code, Physical};
    matches!(
        physical_key,
        Physical::Code(
            Code::ShiftLeft
                | Code::ShiftRight
                | Code::ControlLeft
                | Code::ControlRight
                | Code::AltLeft
                | Code::AltRight
                | Code::CapsLock
        )
    )
}

/// Maps an iced physical key code to the rdev::Key debug string used in hotkey storage.
pub(crate) fn iced_code_to_rdev_key_name(physical_key: &keyboard::key::Physical) -> Option<String> {
    use keyboard::key::{Code, Physical};
    let code = match physical_key {
        Physical::Code(c) => c,
        _ => return None,
    };
    let name = match code {
        Code::KeyA => "KeyA",
        Code::KeyB => "KeyB",
        Code::KeyC => "KeyC",
        Code::KeyD => "KeyD",
        Code::KeyE => "KeyE",
        Code::KeyF => "KeyF",
        Code::KeyG => "KeyG",
        Code::KeyH => "KeyH",
        Code::KeyI => "KeyI",
        Code::KeyJ => "KeyJ",
        Code::KeyK => "KeyK",
        Code::KeyL => "KeyL",
        Code::KeyM => "KeyM",
        Code::KeyN => "KeyN",
        Code::KeyO => "KeyO",
        Code::KeyP => "KeyP",
        Code::KeyQ => "KeyQ",
        Code::KeyR => "KeyR",
        Code::KeyS => "KeyS",
        Code::KeyT => "KeyT",
        Code::KeyU => "KeyU",
        Code::KeyV => "KeyV",
        Code::KeyW => "KeyW",
        Code::KeyX => "KeyX",
        Code::KeyY => "KeyY",
        Code::KeyZ => "KeyZ",
        Code::Digit0 => "Num0",
        Code::Digit1 => "Num1",
        Code::Digit2 => "Num2",
        Code::Digit3 => "Num3",
        Code::Digit4 => "Num4",
        Code::Digit5 => "Num5",
        Code::Digit6 => "Num6",
        Code::Digit7 => "Num7",
        Code::Digit8 => "Num8",
        Code::Digit9 => "Num9",
        Code::F1 => "F1",
        Code::F2 => "F2",
        Code::F3 => "F3",
        Code::F4 => "F4",
        Code::F5 => "F5",
        Code::F6 => "F6",
        Code::F7 => "F7",
        Code::F8 => "F8",
        Code::F9 => "F9",
        Code::F10 => "F10",
        Code::F11 => "F11",
        Code::F12 => "F12",
        Code::ArrowLeft => "LeftArrow",
        Code::ArrowRight => "RightArrow",
        Code::ArrowUp => "UpArrow",
        Code::ArrowDown => "DownArrow",
        Code::Space => "Space",
        Code::Enter => "Return",
        Code::Tab => "Tab",
        Code::Escape => "Escape",
        Code::Backspace => "Backspace",
        Code::Insert => "Insert",
        Code::Delete => "Delete",
        Code::Home => "Home",
        Code::End => "End",
        Code::PageUp => "PageUp",
        Code::PageDown => "PageDown",
        Code::CapsLock => "CapsLock",
        Code::NumLock => "NumLock",
        Code::ScrollLock => "ScrollLock",
        Code::PrintScreen => "PrintScreen",
        Code::Pause => "Pause",
        Code::Quote => "Quote",
        Code::Semicolon => "SemiColon",
        Code::Comma => "Comma",
        Code::Period => "Dot",
        Code::Slash => "Slash",
        Code::Backquote => "BackQuote",
        Code::BracketLeft => "LeftBracket",
        Code::BracketRight => "RightBracket",
        Code::Minus => "Minus",
        Code::Equal => "Equal",
        Code::Backslash => "BackSlash",
        Code::IntlBackslash => "IntlBackslash",
        _ => return None,
    };
    Some(name.to_string())
}
