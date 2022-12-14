use serde::{Deserialize, Serialize};

use super::command::utils::normalized_command::NormalizedCommand;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Keybind {
    pub command: NormalizedCommand,
    pub modifier: Vec<String>,
    pub key: String,
}

/// A trait which can convert self into the `Keybind` struct of lefthk_core
/// by simulating the `self`-keybinding by lefthk_core-keybinds.
pub trait CoreKeybind {
    fn to_core_keybind(&self) -> Vec<Keybind>;
}
