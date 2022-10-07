mod keybind;
pub mod command;

pub use command::Command;
pub use keybind::Keybind;

pub trait Config {
    fn mapped_bindings(&self) -> Vec<Keybind>;
}

pub trait CommandAdapter {
    fn convert(&self) -> Vec<Box<dyn Command>>;
}
