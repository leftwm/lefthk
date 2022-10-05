mod chord;
mod execute;
mod exit_chord;
mod kill;
mod reload;

use std::convert::TryFrom;

use crate::errors::LeftError;

pub use self::{chord::Chord, execute::Execute, exit_chord::ExitChord, reload::Reload};

pub trait Command<'a>: ToString + Default + TryFrom<&'a str> {}

pub static COMMANDS: Vec<Box<dyn Command<Error = LeftError>>> = Vec::new();

#[macro_export]
macro_rules! register_command {
    ($($x:expr),*) =>  {
        {
            COMMANDS.push($x::default());
        }
    }
}
