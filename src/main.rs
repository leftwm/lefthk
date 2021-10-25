use crate::{errors::LeftError, worker::Worker};

mod tests;

pub mod config;
pub mod errors;
pub mod ipc;
pub mod worker;
pub mod xkeysym_lookup;
pub mod xwrap;

fn main() {
    log::info!("lefthk booted!");
    pretty_env_logger::init();
    let completed = std::panic::catch_unwind(|| {
        let rt = errors::return_on_error!(tokio::runtime::Runtime::new());
        let _rt_guard = rt.enter();

        let keybinds = errors::return_on_error!(config::load());
        rt.block_on(Worker::new(keybinds).event_loop());
    });

    match completed {
        Ok(_) => log::info!("Completed"),
        Err(err) => log::error!("Completed with error: {:?}", err),
    }
}
