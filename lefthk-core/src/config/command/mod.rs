mod chord;
mod execute;
mod exit_chord;
mod kill;
mod reload;

pub mod error;

use serde::{Serialize, Deserialize};

use crate::worker::Worker;

pub use self::{chord::Chord, execute::Execute, exit_chord::ExitChord, kill::Kill, reload::Reload};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeneralCommand(String);

pub trait Command {
    fn generalize(&self) -> GeneralCommand;

    fn from_generalized(generalized: GeneralCommand) -> Option<Box<Self>>;

    fn execute(&self, worker: &mut Worker);
}

pub fn from_general<'a>(general: GeneralCommand) -> impl Command {
    todo!()
}
