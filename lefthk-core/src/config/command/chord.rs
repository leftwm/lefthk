use serde::{Deserialize, Serialize};

use crate::{
    config::{command::utils::denormalize_function::DenormalizeCommandFunction, Keybind},
    errors::Error,
    worker::Worker,
};

use super::{Command, NormalizedCommand};

inventory::submit! {DenormalizeCommandFunction::new::<Chord>()}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Chord(Vec<Keybind>);

impl Chord {
    pub fn new(keybinds: Vec<Keybind>) -> Self {
        Self(keybinds)
    }
}

impl Command for Chord {
    fn execute(self, worker: &mut Worker) -> Error {
        worker.xwrap.grab_keys(&self.0);
        worker.chord_ctx.keybinds = Some(self.0.clone());
        Ok(())
    }

    fn normalize(&self) -> NormalizedCommand {
        NormalizedCommand(ron::to_string(self).unwrap())
    }

    fn denormalize(generalized: &NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }
}
