use std::hash::Hash;

use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::{
    config::command::utils::denormalize_function::DenormalizeCommandFunction,
    errors::Error,
    worker::{self, Worker},
};

use super::{Command, NormalizedCommand};

inventory::submit! {DenormalizeCommandFunction::new::<Kill>()}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize, Default)]
pub struct Kill;

impl Kill {
    pub fn new() -> Self {
        Self
    }
}

impl Command for Kill {
    fn normalize(&self) -> NormalizedCommand {
        let serialized_string =
            ron::ser::to_string_pretty(self, PrettyConfig::new().struct_names(true)).unwrap();

        NormalizedCommand(serialized_string)
    }

    fn denormalize(generalized: &NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }

    fn execute(&self, worker: &mut Worker) -> Error {
        worker.status = worker::Status::Kill;
        Ok(())
    }

    fn get_name(&self) -> &'static str {
        "Kill"
    }
}

#[cfg(test)]
mod testes {
    use crate::config::Command;

    use super::Kill;

    #[test]
    fn normalize_process() {
        let command = Kill::new();

        let normalized = command.normalize();
        let denormalized = Kill::denormalize(&normalized).unwrap();

        assert_eq!(
            Box::new(command.clone()),
            denormalized,
            "{:?}, {:?}",
            command,
            denormalized
        );
    }
}
