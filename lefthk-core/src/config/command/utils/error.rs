#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CommandError {
    #[error("Given String doesn't match.")]
    UnmatchingCommand,
}
