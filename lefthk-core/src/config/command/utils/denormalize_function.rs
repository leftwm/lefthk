use crate::config::Command;

use super::normalized_command::NormalizedCommand;

pub struct DenormalizeCommandFunction(pub fn(&NormalizedCommand) -> Option<Box<dyn Command>>);

impl DenormalizeCommandFunction {
    pub const fn new<T: Command + 'static>() -> Self {
        DenormalizeCommandFunction(|normalized: &NormalizedCommand| {
            T::denormalize(normalized).map(|cmd| cmd as Box<dyn Command>)
        })
    }
}
