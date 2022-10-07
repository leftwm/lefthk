use serde::{Serialize, Deserialize};

use crate::{worker::Worker, errors::Error};

use super::{Command, NormalizedCommand};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reload;

impl Reload {
    pub fn new() -> Self {
        Self
    }
}

impl Command for Reload {
    fn execute(&self, worker: &mut Worker) -> Error {
        worker.reload_ctx.requested = true;
        Ok(())
    }

    fn normalize(&self) -> NormalizedCommand {
        NormalizedCommand(ron::to_string(self).unwrap())
    }

    fn denormalize(generalized: NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }
}
