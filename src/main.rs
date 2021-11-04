use crate::{errors::LeftError, worker::Worker};
use clap::{App, Arg};

mod tests;

pub mod config;
pub mod errors;
pub mod ipc;
pub mod worker;
pub mod xkeysym_lookup;
pub mod xwrap;
use std::fs;
use std::io::Write;
use xdg::BaseDirectories;

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
}

fn send_command(command: &str) {
    let path = errors::exit_on_error!(BaseDirectories::with_prefix("lefthk"));
    let pipe_file = errors::exit_on_error!(path.place_runtime_file("commands.pipe"));
    let mut pipe = fs::OpenOptions::new().write(true).open(&pipe_file).unwrap();
    writeln!(&mut pipe, "{}", command).unwrap();
}
