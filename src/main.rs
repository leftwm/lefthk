use crate::worker::Worker;

pub mod config;
pub mod errors;
pub mod ipc;
pub mod worker;
pub mod xkeysym_lookup;
pub mod xwrap;

fn main() {
    log::info!("lefthk booted!");
    pretty_env_logger::init();
    loop {
        let completed = std::panic::catch_unwind(|| {
            let rt = tokio::runtime::Runtime::new().expect("ERROR: couldn't init Tokio runtime");
            let _rt_guard = rt.enter();

            let config = match config::load() {
                Ok(config) => config,
                Err(err) => {
                    log::error!("{} Exiting program.", err);
                    return;
                }
            };
            let mut worker = Worker::new(config.keybind);

            rt.block_on(worker.event_loop());

            if worker.kill_requested {
                log::info!("Exiting.");
                std::process::exit(0);
            }
        });

        match completed {
            Ok(_) => log::info!("Completed"),
            Err(err) => log::error!("Completed with error: {:?}", err),
        }
    }
}
