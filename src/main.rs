use crate::{config::Keybind, errors::LeftError, worker::Worker};

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
        let mut old_keybinds: Option<Vec<Keybind>> = None;

        loop {
            let keybinds = match config::load() {
                Ok(keybinds) => keybinds,
                Err(err) => {
                    let keybinds = match old_keybinds {
                        Some(keybinds) => keybinds,
                        None => {
                            log::error!("Exiting program due to error: {}", err);
                            std::process::exit(1);
                        }
                    };
                    keybinds
                }
            };
            let mut worker = Worker::new(keybinds.clone());

            rt.block_on(worker.event_loop());

            if worker.kill_requested {
                log::info!("Exiting.");
                std::process::exit(0);
            }
            old_keybinds = Some(keybinds);
        }
    });

    match completed {
        Ok(_) => log::info!("Completed"),
        Err(err) => log::error!("Completed with error: {:?}", err),
    }
}
