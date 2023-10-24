use serde::{Deserialize, Serialize};

use crate::config::keybind::Keybind;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum Command {
    Chord(Vec<Keybind>),
    Execute(String),
    Executes(Vec<String>),
    ExitChord,
    Reload,
    Kill,
}
