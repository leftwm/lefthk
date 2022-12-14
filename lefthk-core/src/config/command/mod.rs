mod chord;
mod execute;
mod exit_chord;
mod kill;
mod reload;

pub mod utils;

use self::utils::{
    denormalize_function::DenormalizeCommandFunction, normalized_command::NormalizedCommand,
};
use crate::errors::{Error, LeftError, Result};
use crate::worker::Worker;

pub use self::{chord::Chord, execute::Execute, exit_chord::ExitChord, kill::Kill, reload::Reload};

inventory::collect!(DenormalizeCommandFunction);

/// When adding a command:
///  - a command has to submit itself to the inventory
///  - write a test that it's conversion between normalizel and denormalize works
pub trait Command: std::fmt::Debug {
    fn normalize(&self) -> NormalizedCommand;

    fn denormalize(generalized: &NormalizedCommand) -> Option<Box<Self>>
    where
        Self: Sized;

    /// # Errors
    ///
    /// This errors when the command cannot be executed by the worker
    fn execute(&self, worker: &mut Worker) -> Error;

    fn get_name(&self) -> &'static str;
}

/// # Errors
///
/// This errors when the command cannot be matched with the known commands
pub fn denormalize(normalized_command: &NormalizedCommand) -> Result<Box<dyn Command>> {
    for denormalizer in inventory::iter::<DenormalizeCommandFunction> {
        if let Some(denormalized_command) = (denormalizer.0)(normalized_command) {
            return Ok(denormalized_command);
        }
    }
    Err(LeftError::UnmatchingCommand)
}
