use serde::{Deserialize, Serialize};

use crate::{worker::Worker, errors::Error};

use super::{Command, NormalizedCommand};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct ExitChord;

impl ExitChord {
    pub fn new() -> Self {
        Self
    }
}

impl Command for ExitChord {
    fn execute(&self, worker: &mut Worker) -> Error {
        if worker.chord_ctx.keybinds.is_some() {
            worker.chord_ctx.elapsed = true;
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
