mod tests;

pub mod child;
pub mod config;
pub mod errors;
pub mod evdev;
pub mod ipc;
pub mod worker;
pub mod xkeysym_lookup;
pub mod xwrap;

/// The directory name for xdg
pub const LEFTHK_DIR_NAME: &str = "lefthk";
