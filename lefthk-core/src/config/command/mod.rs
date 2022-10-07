mod chord;
mod execute;
mod exit_chord;
mod kill;
mod reload;

pub mod normalized_command;
pub mod error;

use crate::errors::Error;
use crate::worker::Worker;

pub use self::{error::CommandError, normalized_command::NormalizedCommand};
pub use self::{chord::Chord, execute::Execute, exit_chord::ExitChord, kill::Kill, reload::Reload};

//inventory::collect!(dyn Fn(NormalizedCommand) -> Option<Box<dyn Command>>);

pub type CommandId = u32;

pub trait Command {
    fn normalize(&self) -> NormalizedCommand;

    fn denormalize(generalized: NormalizedCommand) -> Option<Box<Self>>
    where
        Self: Sized;

    fn execute(&self, worker: &mut Worker) -> Error;
}

pub fn denormalize<'a>(general: NormalizedCommand) -> Result<Box<dyn Command>, CommandError> {
    for command in inventory::iter::<Box<dyn Command>> {
        if let Some(matching_command) = command.from_general(general) {
            return Ok(matching_command);
        }
    }

    Err(CommandError::UnmatchingCommand)
}
