use serde::{Serialize, Deserialize};

use super::command::utils::normalized_command::NormalizedCommand;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Keybind {
    pub command: NormalizedCommand,
    pub modifier: Vec<String>,
    pub key: String,
}

