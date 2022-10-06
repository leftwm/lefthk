use serde::{Serialize, Deserialize};

use crate::worker::Worker;

use super::{Command, GeneralCommand};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Execute(String);

impl Execute {
    pub fn new<T: ToString>(shell_command: T) -> Self {
        Self(shell_command.to_string())
    }
}

impl Command for Execute {
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
