use serde::{Serialize, Deserialize};

use super::command::GeneralCommand;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Keybind {
    pub command: GeneralCommand,
    pub modifier: Vec<String>,
    pub key: String,
}

