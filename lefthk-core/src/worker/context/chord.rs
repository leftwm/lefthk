use crate::config::Keybind;

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    pub keybinds: Option<Vec<Keybind>>,
    pub elapsed: bool,
}

impl Chord {
    pub fn new() -> Self {
        Self {
            keybinds: None,
            elapsed: false,
        }
    }
}
