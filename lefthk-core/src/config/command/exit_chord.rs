use serde::{Deserialize, Serialize};

use crate::worker::Worker;

use super::{Command, GeneralCommand};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct ExitChord;

impl ExitChord {
    pub fn new() -> Self {
        Self
    }
}

impl Command for ExitChord {
    fn execute(&self, worker: &mut Worker) {
        todo!()
    }

    fn generalize(&self) -> GeneralCommand {
        GeneralCommand(ron::to_string(self).unwrap())
    }

    fn from_generalized(generalized: GeneralCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }
}
