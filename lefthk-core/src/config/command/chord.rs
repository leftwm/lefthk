use serde::{Deserialize, Serialize};

use crate::{config::Keybind, errors::Error};

use super::{Command, NormalizedCommand};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Chord(Vec<Keybind>);

impl Chord {
    pub fn new(keybinds: Vec<Keybind>) -> Self {
        Self(keybinds)
    }
}

impl Command for Chord {
    fn execute(&self, worker: &mut crate::worker::Worker) -> Error {
        worker.chord_ctx.keybinds = Some(self.0);
        if let Some(keybinds) = worker.chord_ctx.keybinds {
            worker.xwrap.grab_keys(&keybinds);
        }
        Ok(())
    }

    fn normalize(&self) -> NormalizedCommand {
        NormalizedCommand(ron::to_string(self).unwrap())
    }

    fn denormalize(generalized: NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }
}
