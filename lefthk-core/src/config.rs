use crate::errors::{LeftError, Result};
use std::convert::TryFrom;

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Chord(Vec<Keybind>),
    Execute(String),
    ExitChord,
    Reload,
    Kill,
}

impl TryFrom<&str> for Command {
    type Error = LeftError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "Execute" => build_execute(value),
            "ExitChord" => Ok(Self::ExitChord),
            "Reload" => Ok(Self::Reload),
            "Kill" => Ok(Self::Kill),
            _ => Err(LeftError::CommandNotFound),
        }
    }
}

fn build_execute(raw: &str) -> Result<Command> {
    let headless = without_head(raw, "Execute ");
    Ok(Command::Execute(headless.to_owned()))
}

fn without_head<'a, 'b>(s: &'a str, head: &'b str) -> &'a str {
    if !s.starts_with(head) {
        return s;
    }
    &s[head.len()..]
}

#[derive(Debug, PartialEq, Clone)]
pub struct Keybind {
    pub command: Command,
    pub modifier: Vec<String>,
    pub key: String,
}

pub trait Config {
    fn mapped_bindings(&self) -> Vec<Keybind>;
}

#[cfg(feature = "watcher")]
pub mod watcher {
    use crate::errors::{Error, Result};
    use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
    use std::os::unix::prelude::AsRawFd;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::{oneshot, Notify};
    use tokio::time::Duration;

    pub struct Watcher {
        fd: Inotify,
        task_notify: Arc<Notify>,
        _task_guard: oneshot::Receiver<()>,
    }

    impl Watcher {
        pub fn new(config_file: &Path) -> Result<Watcher> {
            const INOTIFY: mio::Token = mio::Token(0);
            let fd = Inotify::init(InitFlags::all())?;
            let mut flags = AddWatchFlags::empty();
            flags.insert(AddWatchFlags::IN_MODIFY);
            let _wd = fd.add_watch(config_file, flags)?;

            let (guard, _task_guard) = oneshot::channel::<()>();
            let notify = Arc::new(Notify::new());
            let task_notify = notify.clone();
            let mut poll = mio::Poll::new()?;
            let mut events = mio::Events::with_capacity(1);
            poll.registry().register(
                &mut mio::unix::SourceFd(&fd.as_raw_fd()),
                INOTIFY,
                mio::Interest::READABLE,
            )?;
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
            Ok(Self {
                fd,
                task_notify,
                _task_guard,
            })
        }

        pub fn refresh_watch(&self, config_file: &Path) -> Error {
            let mut flags = AddWatchFlags::empty();
            flags.insert(AddWatchFlags::IN_MODIFY);
            let _wd = self.fd.add_watch(config_file, flags)?;
            Ok(())
        }

        pub fn has_events(&self) -> bool {
            self.fd.read_events().is_ok()
        }

        /// Wait until readable.
        pub async fn wait_readable(&mut self) {
            self.task_notify.notified().await;
        }
    }
}
