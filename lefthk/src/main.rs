use crate::errors::LeftError;
use clap::{App, Arg};
use lefthk_core::config::{Command, command};
use lefthk_core::ipc::Pipe;
use lefthk_core::worker::Status;
use lefthk_core::{config::Config, worker::Worker};
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use xdg::BaseDirectories;

use tracing_subscriber::{filter::EnvFilter, filter::LevelFilter, fmt, layer::SubscriberExt};

pub mod config;
pub mod errors;
mod tests;

const QUIT_COMMAND: &str = "quit";
const RELOAD_COMMAND: &str = "reload";

fn main() {
    setup_logging();
    let app = get_app();
    let matches = app.get_matches();
    tracing::info!("lefthk booted!");

    if matches.contains_id(QUIT_COMMAND) {
        send_command(command::Kill);
    } else if matches.contains_id(RELOAD_COMMAND) {
        send_command(command::Reload);
    } else {
        let mut old_config = None;
        let path =
            errors::exit_on_error!(BaseDirectories::with_prefix(lefthk_core::LEFTHK_DIR_NAME));
        loop {
            let config = match config::load() {
                Ok(config) => config,
                Err(err) => match old_config {
                    Some(config) => config,
                    None => {
                        tracing::error!("Unable to load new config due to error: {}", err);
                        return;
                    }
                },
            };
            let kill_requested = AtomicBool::new(false);
            let completed = std::panic::catch_unwind(|| {
                let rt = errors::return_on_error!(tokio::runtime::Runtime::new());
                let _rt_guard = rt.enter();

                let status =
                    rt.block_on(Worker::new(config.mapped_bindings(), path.clone()).event_loop());
                kill_requested.store(status == Status::Kill, Ordering::SeqCst);
            });

            match completed {
                Ok(_) => tracing::info!("Completed"),
                Err(err) => tracing::error!("Completed with error: {:?}", err),
            }
            if kill_requested.load(Ordering::SeqCst) {
                return;
            }
            old_config = Some(config);
        }
    }
}

fn send_command(command: impl Command) {
    let path = errors::exit_on_error!(BaseDirectories::with_prefix(lefthk_core::LEFTHK_DIR_NAME));
    let pipe_name = Pipe::pipe_name();
    let pipe_file = errors::exit_on_error!(path.place_runtime_file(pipe_name));
    let mut pipe = fs::OpenOptions::new().write(true).open(&pipe_file).unwrap();
    writeln!(pipe, "{}", command.normalize()).unwrap();
}

fn get_app() -> App<'static> {
    clap::command!()
        .arg(
            Arg::with_name(QUIT_COMMAND)
                .short('q')
                .long(QUIT_COMMAND)
                .help("Quit a running daemon instance"),
        )
        .arg(
            Arg::with_name(RELOAD_COMMAND)
                .short('r')
                .long(RELOAD_COMMAND)
                .help("Reload daemon to apply changes to config"),
        )
}

fn setup_logging() {
    let subscriber = fmt::Layer::new().with_writer(std::io::stdout);
    let log_level = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env_lossy();

    let collector = tracing_subscriber::registry()
        .with(log_level)
        .with(subscriber);

    tracing::subscriber::set_global_default(collector).expect("Couldn't setup logging");
}
