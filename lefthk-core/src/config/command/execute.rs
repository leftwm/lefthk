use std::process::Stdio;

use serde::{Serialize, Deserialize};

use crate::{worker::Worker, errors::Error};

use super::{Command, NormalizedCommand};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Execute(String);

impl Execute {
    pub fn new<T: ToString>(shell_command: T) -> Self {
        Self(shell_command.to_string())
    }
}

impl Command for Execute {
    fn execute(&self, worker: &mut Worker) -> Error {
        worker.chord_ctx.elapsed = worker.chord_ctx.keybinds.is_some();
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg(&self.0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?;

        worker.children.insert(child);

        Ok(())
    }

    fn normalize(&self) -> NormalizedCommand {
        NormalizedCommand(ron::to_string(self).unwrap())
    }

    fn denormalize(generalized: NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }
}
