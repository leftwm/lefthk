use serde::{Deserialize, Serialize};

use crate::config::Keybind;

use super::{Command, GeneralCommand};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Chord(Vec<Keybind>);

impl Chord {
    pub fn new(keybinds: Vec<Keybind>) -> Self {
        Self(keybinds)
    }
}

impl Command for Chord {
    fn execute(&self, worker: &mut crate::worker::Worker) {
        todo!()
    }

    fn generalize(&self) -> GeneralCommand {
        GeneralCommand(ron::to_string(self).unwrap())
    }

    fn from_generalized(generalized: GeneralCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }
}
