use crate::config::Keybind;

use super::Command;

#[derive(Debug, PartialEq)]
pub struct Chord(Vec<Keybind>);

impl Chord {
    pub fn new(keybinds: Vec<Keybind>) -> Self {
        Self(keybinds)
        

    }
}

impl<'a> Command<'a> for Chord {
}
