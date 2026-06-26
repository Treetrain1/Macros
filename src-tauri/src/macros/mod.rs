use crate::input::types::InputToken;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) mod backend;
pub(crate) mod priority;
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
    pub(crate) name: String,
    pub(crate) description: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "InstructionDe")]
pub(crate) enum Instruction {
    Token(InputToken),
    Wait(f64, f64),
    Command(String),
    Comment(String),
}

impl std::hash::Hash for Instruction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Token(t)   => { 0u8.hash(state); t.hash(state); }
            Self::Wait(d, r) => { 1u8.hash(state); d.to_bits().hash(state); r.to_bits().hash(state); }
            Self::Command(s) => { 2u8.hash(state); s.hash(state); }
            Self::Comment(s) => { 3u8.hash(state); s.hash(state); }
        }
    }
}

#[derive(Deserialize)]
enum InstructionDe {
    Token(InputToken),
    Wait(WaitDe),
    Command(String),
    Comment(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WaitDe {
    Legacy(u64),
    Current(f64, f64),
}

impl From<InstructionDe> for Instruction {
    fn from(de: InstructionDe) -> Self {
        match de {
            InstructionDe::Token(t)                    => Instruction::Token(t),
            InstructionDe::Wait(WaitDe::Legacy(d))     => Instruction::Wait(d as f64, 0.0),
            InstructionDe::Wait(WaitDe::Current(d, r)) => Instruction::Wait(d, r),
            InstructionDe::Command(s)                  => Instruction::Command(s),
            InstructionDe::Comment(s)                  => Instruction::Comment(s),
        }
    }
}
