use clap::Arg;

use super::keybind::Keybind;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Command {
    Chord(Vec<Keybind>),
    Execute(String),
    Executes(Vec<String>),
    ExitChord,
    Reload,
    Kill,
}
