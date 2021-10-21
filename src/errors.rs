use thiserror::Error;

pub type Result<T> = std::result::Result<T, LeftError>;
pub type Error = std::result::Result<(), LeftError>;

#[derive(Debug, Error)]
pub enum LeftError {
    #[error("IO error: {0}.")]
    IoError(#[from] std::io::Error),
    #[error("Kdl error: {0}.")]
    KdlError(#[from] kdl::KdlError),
    #[error("XDG error: {0}.")]
    XdgBaseDirError(#[from] xdg::BaseDirectoriesError),

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
    #[error("Config watcher dropped.")]
    WatcherDropped,
    #[error("X failed status error.")]
    XFailedStatus,
}
