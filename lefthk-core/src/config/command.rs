use super::{build_execute, Keybind};
use crate::errors::{LeftError, Result};
use std::convert::TryFrom;

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Chord(Vec<Keybind>),
    Execute(String),
    ExitChord,
    Reload,
    Kill,
}

impl TryFrom<&str> for Command {
    type Error = LeftError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "Execute" => build_execute(value),
            "ExitChord" => Ok(Self::ExitChord),
            "Reload" => Ok(Self::Reload),
            "Kill" => Ok(Self::Kill),
            _ => Err(LeftError::CommandNotFound),
        }
    }
}
