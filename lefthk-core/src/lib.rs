mod tests;

pub mod child;
pub mod config;
pub mod errors;
pub mod evdev;
pub mod ipc;
pub mod keysym_lookup;
pub mod worker;

/// The directory name for xdg
pub const LEFTHK_DIR_NAME: &str = "lefthk";
