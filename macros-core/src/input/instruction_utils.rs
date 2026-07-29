use crate::input::types::{Axis, Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::Value;
use crate::macros::Instruction;

pub fn create_default_instruction(instruction_type: usize) -> Option<Instruction> {
    match instruction_type {
        0 => Some(Instruction::Wait(Value::number(1000.0))),
        1 => Some(Instruction::Token(InputToken::Text(Value::Text { value: "text".into() }))),
        2 => Some(Instruction::Token(InputToken::Key(MacroKey::Unicode('a'), Direction::Click))),
        3 => Some(Instruction::Token(InputToken::Button(MacroButton::Left, Direction::Click))),
        4 => Some(Instruction::Token(InputToken::MoveMouse(Value::number(0.0), Value::number(0.0), Coordinate::Rel))),
        5 => Some(Instruction::Token(InputToken::Scroll(Value::number(4.0), Axis::Vertical))),
        6 => Some(Instruction::Command("script".into())),
        _ => None,
    }
}

pub fn get_instruction_type_names() -> &'static [&'static str] {
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
