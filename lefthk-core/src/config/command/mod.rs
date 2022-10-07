mod chord;
mod execute;
mod exit_chord;
mod kill;
mod reload;

pub mod utils;

use crate::errors::Error;
use crate::worker::Worker;
use self::utils::{
    denormalize_function::DenormalizeCommandFunction, error::CommandError,
    normalized_command::NormalizedCommand,
};

pub use self::{chord::Chord, execute::Execute, exit_chord::ExitChord, kill::Kill, reload::Reload};

inventory::collect!(DenormalizeCommandFunction);

pub trait Command {
    fn normalize(&self) -> NormalizedCommand;

    fn denormalize(generalized: &NormalizedCommand) -> Option<Box<Self>>
    where
        Self: Sized;

    fn execute(self, worker: &mut Worker) -> Error;
}

pub fn denormalize<'a>(normalized_command: NormalizedCommand) -> Result<Box<dyn Command>, CommandError> {
    for denormalizer in inventory::iter::<DenormalizeCommandFunction> {
        if let Some(denormalized_command) = (denormalizer.0)(&normalized_command) {
            return Ok(denormalized_command);
        }
    }
    Err(CommandError::UnmatchingCommand)
}
