use std::hash::Hash;

use serde::{Serialize, Deserialize};

use crate::{worker::{Worker, self}, errors::Error};

use super::{Command, NormalizedCommand};

#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct Kill;

impl Kill {
    pub fn new() -> Self {
        Self
    }
}

impl Command for Kill {
    fn execute(&self, worker: &mut Worker) -> Error {
        worker.status = worker::Status::Kill;
        Ok(())
    }

    fn normalize(&self) -> NormalizedCommand {
        NormalizedCommand(ron::to_string(self).unwrap())
    }

    fn denormalize(generalized: NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }
}
