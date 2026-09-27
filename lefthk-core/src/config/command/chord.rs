use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::{
    config::{Keybind, command::utils::denormalize_function::DenormalizeCommandFunction},
    errors::Error,
    worker::{StatefullKeybind, Worker},
};

use super::{Command, NormalizedCommand};

inventory::submit! {DenormalizeCommandFunction::new::<Chord>()}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord(Vec<Keybind>);

impl Chord {
    #[must_use]
    pub fn new(keybinds: Vec<Keybind>) -> Self {
        Self(keybinds)
    }
}

impl Command for Chord {
    fn normalize(&self) -> NormalizedCommand {
        let serialized_string =
            ron::ser::to_string_pretty(self, PrettyConfig::new().struct_names(true)).unwrap();
        NormalizedCommand(serialized_string)
    }

    fn denormalize(generalized: &NormalizedCommand) -> Option<Box<Self>> {
        ron::from_str(&generalized.0).ok()
    }

    fn execute(&self, worker: &mut Worker) -> Error {
        let sf_keybinds = self
            .0
            .clone()
            .into_iter()
            .map(StatefullKeybind::new)
            .collect::<Vec<StatefullKeybind>>();
        worker.xwrap.grab_keys(&sf_keybinds);
        worker.chord_ctx.sf_keybinds = Some(sf_keybinds);
        Ok(())
    }

    fn get_name(&self) -> &'static str {
        "Chord"
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Command, Keybind, command::Reload};

    use super::Chord;

    #[test]
    fn normalize_process() {
        let command = Chord::new(vec![Keybind {
            command: Reload::new().normalize(),
            modifier: vec![],
            key: String::new(),
            on_release: false,
        }]);

        let normalized = command.normalize();
        let denormalized = Chord::denormalize(&normalized).unwrap();

        assert_eq!(
            Box::new(command.clone()),
            denormalized,
            "{command:?}, {denormalized:?}",
        );
    }
}
