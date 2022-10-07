use serde::{Serialize, Deserialize};

use super::command::NormalizedCommand;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Keybind {
    pub command: NormalizedCommand,
    pub modifier: Vec<String>,
    pub key: String,
}

