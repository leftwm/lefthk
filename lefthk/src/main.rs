use crate::errors::LeftError;
use clap::{App, Arg};
use lefthk_core::ipc::Pipe;
use lefthk_core::{config::Config, worker::Worker};
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use xdg::BaseDirectories;

pub mod config;
pub mod errors;
mod tests;

fn main() {
    let matches = App::new("LeftHK Hot Key Daemon")
        .about("a simple hotkey daemon for LeftWM")
        .arg(
            Arg::with_name("quit")
                .short("q")
                .long("quit")
                .help("Quit a running daemon instance"),
        )
        .arg(
            Arg::with_name("reload")
                .short("r")
                .long("reload")
                .help("Reload daemon to apply changes to config"),
        )
        .get_matches();
    log::info!("lefthk booted!");

    if matches.occurrences_of("quit") == 1 {
        send_command("Kill");
    } else if matches.occurrences_of("reload") == 1 {
        send_command("Reload");
    } else {
        pretty_env_logger::init();
        let mut old_config = None;
        let path = errors::exit_on_error!(BaseDirectories::with_prefix("lefthk"));
        #[cfg(feature = "watcher")]
        let config_file = errors::exit_on_error!(path.place_config_file("config.kdl"));
        loop {
            let config = match config::load() {
                Ok(config) => config,
                Err(err) => match old_config {
                    Some(config) => config,
                    None => {
                        log::error!("Unable to load new config due to error: {}", err);
                        return;
                    }
                },
            };
            let kill_requested = AtomicBool::new(false);
            let completed = std::panic::catch_unwind(|| {
                let rt = errors::return_on_error!(tokio::runtime::Runtime::new());
                let _rt_guard = rt.enter();
                #[cfg(feature = "watcher")]
                let mut worker =
                    Worker::new(config.mapped_bindings(), config_file.clone(), path.clone());

                #[cfg(not(feature = "watcher"))]
                let mut worker = Worker::new(config.mapped_bindings(), path.clone());

                rt.block_on(worker.event_loop());
                kill_requested.store(worker.kill_requested, Ordering::SeqCst);
            });

            match completed {
                Ok(_) => log::info!("Completed"),
                Err(err) => log::error!("Completed with error: {:?}", err),
            }
            if kill_requested.load(Ordering::SeqCst) {
                return;
            }
            old_config = Some(config);
        }
    }
}

fn send_command(command: &str) {
    let path = errors::exit_on_error!(BaseDirectories::with_prefix("lefthk"));
    let pipe_name = Pipe::pipe_name();
    let pipe_file = errors::exit_on_error!(path.place_runtime_file(pipe_name));
    let mut pipe = fs::OpenOptions::new().write(true).open(&pipe_file).unwrap();
    writeln!(pipe, "{}", command).unwrap();
}
