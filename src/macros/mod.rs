use enigo::agent::Token;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) mod runner;
pub(crate) mod thread_pool;
pub(crate) mod thread;
pub(crate) mod loop_control;

fn default_macro_id() -> String {
    Uuid::new_v4().simple().to_string()
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) struct Macro {
    #[serde(default = "default_macro_id")]
    pub(crate) id: String,
    pub(crate) name: String, /// displayed in GUI
    pub(crate) description: String, /// displayed in GUI, TODO: add the description display
    pub(crate) code: Vec<Instruction>,
}

impl Macro {
    pub(crate) fn new(name: String, description: String, code: Vec<Instruction>) -> Self {
        Self {
            id: default_macro_id(),
            name,
            description,
            code,
        }
    }

    pub(crate) fn ensure_id(&mut self) {
        if self.id.trim().is_empty() {
            self.id = default_macro_id();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "InstructionDe")]
pub(crate) enum Instruction {
    Token(Token),
    Wait(u64, u64),
    Command(String),
    Comment(String),
}

#[derive(Deserialize)]
enum InstructionDe {
    Token(Token),
    Wait(WaitDe),
    Command(String),
    Comment(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WaitDe {
    Legacy(u64),
    Current(u64, u64),
}

impl From<InstructionDe> for Instruction {
    fn from(de: InstructionDe) -> Self {
        match de {
            InstructionDe::Token(t)                    => Instruction::Token(t),
            InstructionDe::Wait(WaitDe::Legacy(d))     => Instruction::Wait(d, 0),
            InstructionDe::Wait(WaitDe::Current(d, r)) => Instruction::Wait(d, r),
            InstructionDe::Command(s)                   => Instruction::Command(s),
            InstructionDe::Comment(s)                  => Instruction::Comment(s),
        }
    }
}
