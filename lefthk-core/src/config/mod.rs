pub mod command;
mod keybind;

pub use command::Command;
pub use keybind::{Keybind, CoreKeybind};

pub trait Config {
    fn mapped_keybinds(&self) -> Vec<Keybind>;
}
