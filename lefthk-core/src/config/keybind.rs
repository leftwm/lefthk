use super::Command;

#[derive(Debug, PartialEq, Clone)]
pub struct Keybind {
    pub command: Command,
    pub modifier: Vec<String>,
    pub key: String,
}

