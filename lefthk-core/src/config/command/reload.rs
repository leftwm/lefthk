use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::{
    config::command::utils::denormalize_function::DenormalizeCommandFunction,
    errors::Error,
    worker::{self, Worker},
};

use super::{Command, NormalizedCommand};

inventory::submit! {DenormalizeCommandFunction::new::<Reload>()}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Reload;

impl Reload {
    pub fn new() -> Self {
        Self
    }
}

impl Command for Reload {
    fn normalize(&self) -> NormalizedCommand {
        let serialized_string = ron::ser::to_string_pretty(self, PrettyConfig::new().struct_names(true)).unwrap();
        NormalizedCommand(serialized_string)
    }

    fn denormalize(generalized: &NormalizedCommand) -> Option<Box<Self>> {
        match ron::from_str(&generalized.0) {
            Ok(penis) => Some(penis),
            Err(err) => panic!("Message: {}, Struct: {:?}", err, generalized),
        }
    }

    fn execute(&self, worker: &mut Worker) -> Error {
        worker.status = worker::Status::Reload;
        Ok(())
    }

    fn get_name(&self) -> &'static str {
        "Reload"
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Command;

    use super::Reload;

    #[test]
    fn normalize_process() {
        let command = Reload::new();

        let normalized = command.normalize();
        let denormalized = Reload::denormalize(&normalized).unwrap();

        assert_eq!(Box::new(command), denormalized);
    }
}
