pub mod command;
mod keybind;

pub use command::Command;
pub use keybind::Keybind;

pub trait Config {
    fn mapped_bindings(&self) -> Vec<Keybind>;
}

pub trait CommandAdapter {
    fn convert(&self) -> Vec<Box<dyn Command>>;
}
