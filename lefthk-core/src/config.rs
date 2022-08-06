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

fn build_execute(raw: &str) -> Result<Command> {
    let headless = without_head(raw, "Execute ");
    Ok(Command::Execute(headless.to_owned()))
}

fn without_head<'a, 'b>(s: &'a str, head: &'b str) -> &'a str {
    if !s.starts_with(head) {
        return s;
    }
    &s[head.len()..]
}

#[derive(Debug, PartialEq, Clone)]
pub struct Keybind {
    pub command: Command,
    pub modifier: Vec<String>,
    pub key: String,
}

pub trait Config {
    fn mapped_bindings(&self) -> Vec<Keybind>;
}
