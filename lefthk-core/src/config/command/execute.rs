use std::process::Stdio;

use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::{
    config::command::utils::denormalize_function::DenormalizeCommandFunction, errors::Error,
    worker::Worker,
};

use super::{Command, NormalizedCommand};

inventory::submit! {DenormalizeCommandFunction::new::<Execute>()}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Execute(String);

impl Execute {
    pub fn new<T: ToString>(shell_command: &T) -> Self {
        Self(shell_command.to_string())
    }
}

impl Command for Execute {
    fn normalize(&self) -> NormalizedCommand {
        let serialized_string =
            ron::ser::to_string_pretty(self, PrettyConfig::new().struct_names(true)).unwrap();
        NormalizedCommand(serialized_string)
    }

    fn denormalize(generalized: &NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }

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

    fn get_name(&self) -> &'static str {
        "Execute"
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Command;

    use super::Execute;

    #[test]
    fn normalize_process() {
        let command = Execute::new(&"echo 'I use Arch by the way'");

        let normalized = command.normalize();
        let denormalized = Execute::denormalize(&normalized).unwrap();

        assert_eq!(
            Box::new(command),
            denormalized,
            "{:?}, {:?}",
            normalized,
            denormalized
        );
    }
}
