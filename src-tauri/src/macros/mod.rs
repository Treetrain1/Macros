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

fn default_strand_id() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Fixed id of the one strand that is actually executed when a macro runs.
/// Every macro always has exactly one strand with this id — it can be
/// emptied (by dragging every block off of it) but never removed, so there
/// is always a well-defined "main" strand to attach pre-existing macros to
/// and to run.
pub(crate) const ROOT_STRAND_ID: &str = "root";

/// One draggable stack of instructions on the canvas. Strands that aren't
/// attached to the root strand are still persisted with the macro (just not
/// executed yet) — this is what allows stray/detached strands to survive a
/// save/reload, and lays the groundwork for later running disconnected
/// strands as independent functions.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) struct Strand {
    #[serde(default = "default_strand_id")]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) x: i32,
    #[serde(default)]
    pub(crate) y: i32,
    #[serde(default)]
    pub(crate) instructions: Vec<Instruction>,
}

impl Strand {
    fn root() -> Self {
        Self { id: ROOT_STRAND_ID.to_string(), x: 0, y: 0, instructions: vec![] }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(from = "MacroDe")]
pub(crate) struct Macro {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) strands: Vec<Strand>,
}

/// Deserialization shape supporting both the current multi-strand format and
/// the legacy single flat `code: Vec<Instruction>` format saved by older
/// versions of the app. Whichever variant matches the JSON on disk wins;
/// legacy macros become a single root strand holding their old instructions.
#[derive(Deserialize)]
#[serde(untagged)]
enum MacroDe {
    Current {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        strands: Vec<Strand>,
    },
    Legacy {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        code: Vec<Instruction>,
    },
}

impl From<MacroDe> for Macro {
    fn from(de: MacroDe) -> Self {
        match de {
            MacroDe::Current { id, name, description, mut strands } => {
                if !strands.iter().any(|s| s.id == ROOT_STRAND_ID) {
                    strands.insert(0, Strand::root());
                }
                Self { id, name, description, strands }
            }
            MacroDe::Legacy { id, name, description, code } => {
                let root = Strand { id: ROOT_STRAND_ID.to_string(), x: 0, y: 0, instructions: code };
                Self { id, name, description, strands: vec![root] }
            }
        }
    }
}

impl Macro {
    pub(crate) fn new(name: String, description: String, code: Vec<Instruction>) -> Self {
        let root = Strand { id: ROOT_STRAND_ID.to_string(), x: 0, y: 0, instructions: code };
        Self {
            id: default_macro_id(),
            name,
            description,
            strands: vec![root],
        }
    }

    pub(crate) fn ensure_id(&mut self) {
        if self.id.trim().is_empty() {
            self.id = default_macro_id();
        }
        if !self.strands.iter().any(|s| s.id == ROOT_STRAND_ID) {
            self.strands.insert(0, Strand::root());
        }
    }

    pub(crate) fn root(&self) -> &Strand {
        self.strands.iter().find(|s| s.id == ROOT_STRAND_ID)
            .expect("macro invariant: root strand always present")
    }

    pub(crate) fn root_mut(&mut self) -> &mut Strand {
        self.strands.iter_mut().find(|s| s.id == ROOT_STRAND_ID)
            .expect("macro invariant: root strand always present")
    }

    pub(crate) fn strand(&self, id: &str) -> Option<&Strand> {
        self.strands.iter().find(|s| s.id == id)
    }

    pub(crate) fn strand_mut(&mut self, id: &str) -> Option<&mut Strand> {
        self.strands.iter_mut().find(|s| s.id == id)
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
