mod keybind;
mod command;

use crate::errors::Result;

pub use command::Command;
pub use keybind::Keybind;

fn build_execute(raw: &str) -> Result<Command> {
    let headless = without_head(raw, "Execute ");
    Ok(Command::Execute(headless.to_owned()))
}

fn without_head<'a, 'b>(s: &'a str, head: &'b str) -> &'a str {
    if !s.starts_with(head) {
        return s;
    }
    &s[head.len()..]
}

pub trait Config {
    fn mapped_bindings(&self) -> Vec<Keybind>;
}
