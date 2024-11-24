use enigo::agent::Token;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Instruction {
    Token(Token),
    Wait(u64),
}
