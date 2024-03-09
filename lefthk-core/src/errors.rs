use thiserror::Error;

macro_rules! r#return {
    ($a: expr) => {
        match $a {
            Ok(value) => value,
            Err(err) => {
                tracing::error!("{}", LeftError::from(err));
                return;
            }
        }
    };
}

macro_rules! log {
    ($a: expr) => {
        match $a {
            Ok(value) => value,
            Err(err) => tracing::error!("{}", LeftError::from(err)),
        }
    };
}

macro_rules! exit {
    ($a: expr) => {
        match $a {
            Ok(value) => value,
            Err(err) => {
                tracing::error!("Exiting due to error: {}", LeftError::from(err));
                std::process::exit(1);
            }
        }
    };
}

pub(crate) use exit;
pub(crate) use log;
pub(crate) use r#return;

pub type Result<T> = std::result::Result<T, LeftError>;
pub type Error = std::result::Result<(), LeftError>;

#[derive(Debug, Error)]
pub enum LeftError {
    #[error("IO error: {0}.")]
    IoError(#[from] std::io::Error),
    #[error("Nix errno: {0}.")]
    NixErrno(#[from] nix::errno::Errno),
    #[error("XDG error: {0}.")]
    XdgBaseDirError(#[from] xdg::BaseDirectoriesError),

    #[error("Given String doesn't match with a command.")]
    UnmatchingCommand,
    #[error("No command found for keybind.")]
    CommandNotFound,
    #[error("No key found for keybind.")]
    KeyNotFound,
    #[error("No modifier found for keybind.")]
    ModifierNotFound,
    #[error("No config file found.")]
    NoConfigFound,
    #[error("No value set for execution.")]
    ValueNotFound,
    #[error("UInput device could not be found.")]
    UInputNotFound,
}
