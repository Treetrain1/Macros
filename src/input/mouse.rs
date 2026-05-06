use enigo::Button as EnigoButton;

pub(crate) fn get_mouse_button_names() -> &'static [&'static str] {
    &[
        "Left",
        "Right",
        "Middle",
        "Back",
        "Forward",
        "ScrollUp",
        "ScrollDown",
        "ScrollLeft",
        "ScrollRight",
    ]
}

pub(crate) fn mouse_button_to_index(button: &EnigoButton) -> usize {
    match button {
        EnigoButton::Left => 0,
        EnigoButton::Right => 1,
        EnigoButton::Middle => 2,
        EnigoButton::Back => 3,
        EnigoButton::Forward => 4,
        EnigoButton::ScrollUp => 5,
        EnigoButton::ScrollDown => 6,
        EnigoButton::ScrollLeft => 7,
        EnigoButton::ScrollRight => 8,
    }
}

pub(crate) fn index_to_mouse_button(index: usize) -> EnigoButton {
    match index {
        0 => EnigoButton::Left,
        1 => EnigoButton::Right,
        2 => EnigoButton::Middle,
        3 => EnigoButton::Back,
        4 => EnigoButton::Forward,
        5 => EnigoButton::ScrollUp,
        6 => EnigoButton::ScrollDown,
        7 => EnigoButton::ScrollLeft,
        8 => EnigoButton::ScrollRight,
        _ => EnigoButton::Left,
    }
}
