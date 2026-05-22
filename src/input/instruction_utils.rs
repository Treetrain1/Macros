use crate::macros::Instruction;
use enigo::agent::Token;
use enigo::{Axis, Button, Coordinate, Direction, Key};

pub(crate) fn create_default_instruction(instruction_type: usize) -> Option<Instruction> {
    match instruction_type {
        0 => Some(Instruction::Wait(1000, 0)),
        1 => Some(Instruction::Token(Token::Text("text".into()))),
        2 => Some(Instruction::Token(Token::Key(Key::Unicode('a'), Direction::Click))),
        3 => Some(Instruction::Token(Token::Button(Button::Left, Direction::Click))),
        4 => Some(Instruction::Token(Token::MoveMouse(0, 0, Coordinate::Rel))),
        5 => Some(Instruction::Token(Token::Scroll(4, Axis::Vertical))),
        6 => Some(Instruction::Command("script".into())),
        _ => None,
    }
}

pub(crate) fn get_instruction_type_names() -> &'static [&'static str] {
    &[
        "Wait",
        "Text",
        "Key",
        "Mouse Button",
        "Move Mouse",
        "Scroll",
        "Run Script",
    ]
}
