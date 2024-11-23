use enigo::{Button, Key};

pub(crate) struct Macro {
    name: String, /// displayed in GUI
    description: String, /// displayed in GUI
    pub(crate) code: Vec<Instruction>,
}

impl Macro {
    pub(crate) fn new(name: String, description: String, code: Vec<Instruction>) -> Self {
        Self { name, description, code, }
    }
}

//pub(crate) struct Instruction {
//    pub(crate) instruction_type: InstructionType,
//    pub(crate) duration: i32,
//}

pub(crate) enum Instruction {
    Wait(u64),
    Text(String),
    KeyDown(Key),
    KeyUp(Key),
    ButtonDown(Button),
    ButtonUp(Button),
    MouseMove(i32, i32),
}