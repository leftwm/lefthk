use crate::worker::{StatefullKeybind, Worker};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chord {
    pub sf_keybinds: Option<Vec<StatefullKeybind>>,
    pub elapsed: bool,
}

impl Chord {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sf_keybinds: None,
            elapsed: false,
        }
    }
}

impl Worker {
    pub fn evaluate_chord(&mut self) {
        if self.chord_ctx.elapsed {
            self.xwrap.grab_keys(&self.sf_keybinds);
            self.chord_ctx.sf_keybinds = None;
            self.chord_ctx.elapsed = false;
        }
    }
}
