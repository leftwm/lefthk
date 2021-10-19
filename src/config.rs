use crate::errors::{LeftError, Result};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use serde::{Deserialize, Serialize};
use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;
use std::{fs, path::Path};
use tokio::sync::{oneshot, Notify};
use tokio::time::Duration;
use xdg::BaseDirectories;

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Execute,
    Reload,
    Kill,
}

#[derive(Serialize, Deserialize)]
pub struct Keybind {
    pub command: Command,
    pub value: Option<String>,
    pub modifier: Vec<String>,
    pub key: String,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub keybind: Vec<Keybind>,
}

pub fn load() -> Result<Config> {
    let path = BaseDirectories::with_prefix("lefthk")?;
    fs::create_dir_all(&path.get_config_home())?;
    let file_name = path.place_config_file("config.toml")?;
    if Path::new(&file_name).exists() {
        let contents = fs::read_to_string(file_name)?;
        return toml::from_str::<Config>(&contents).map_err(Into::into);
    }
    Err(LeftError::NoConfigFound)
}

pub struct Watcher {
    fd: Inotify,
    task_notify: Arc<Notify>,
    _task_guard: oneshot::Receiver<()>,
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Watcher {
    pub fn new() -> Watcher {
        const INOTIFY: mio::Token = mio::Token(0);
        let fd = Inotify::init(InitFlags::all()).expect("ERROR: Could not init iNotify.");
        let path =
            BaseDirectories::with_prefix("lefthk").expect("ERROR: Could not find base directory.");
        let file_name = path
            .find_config_file("config.toml")
            .expect("ERROR: Could not find config file.");
        let mut flags = AddWatchFlags::empty();
        flags.insert(AddWatchFlags::IN_MODIFY | AddWatchFlags::IN_CLOSE);
        let _wd = fd
            .add_watch(&file_name, flags)
            .expect("ERROR: Failed to watch config file.");

        let (guard, _task_guard) = oneshot::channel::<()>();
        let notify = Arc::new(Notify::new());
        let task_notify = notify.clone();
        let mut poll = mio::Poll::new().expect("Unable to boot Mio");
        let mut events = mio::Events::with_capacity(1);
        poll.registry()
            .register(
                &mut mio::unix::SourceFd(&fd.as_raw_fd()),
                INOTIFY,
                mio::Interest::READABLE,
            )
            .expect("Unable to boot Mio");
        let timeout = Duration::from_millis(50);
        tokio::task::spawn_blocking(move || loop {
            if guard.is_closed() {
                return;
            }

            if let Err(err) = poll.poll(&mut events, Some(timeout)) {
                log::warn!("Inotify socket poll failed with {:?}", err);
                continue;
            }

            events
                .iter()
                .filter(|event| INOTIFY == event.token())
                .for_each(|_| notify.notify_one());
        });
        Self {
            fd,
            task_notify,
            _task_guard,
        }
    }

    pub fn has_events(&self) -> bool {
        self.fd.read_events().is_ok()
    }

    /// Wait until readable.
    pub async fn wait_readable(&mut self) {
        self.task_notify.notified().await;
    }
}
