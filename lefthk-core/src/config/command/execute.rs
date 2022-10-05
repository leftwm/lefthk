#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Execute(String);

impl Execute {
    pub fn new<T: ToString>(shell_command: T) -> Self {
        Self(shell_command.to_string())
    }
}
