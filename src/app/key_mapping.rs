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
