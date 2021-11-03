use crate::errors::{Error, LeftError, Result};
use kdl::KdlNode;
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use serde::{Deserialize, Serialize};
use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;
use std::{convert::TryFrom, fs, path::Path};
use tokio::sync::{oneshot, Notify};
use tokio::time::Duration;
use xdg::BaseDirectories;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Command {
    Chord,
    Execute,
    ExitChord,
    Reload,
    Kill,
}

impl TryFrom<String> for Command {
    type Error = LeftError;

    fn try_from(value: String) -> Result<Self> {
        match value {
            s if s == "Chord" => Ok(Self::Chord),
            s if s == "Execute" => Ok(Self::Execute),
            s if s == "ExitChord" => Ok(Self::ExitChord),
            s if s == "Reload" => Ok(Self::Reload),
            s if s == "Kill" => Ok(Self::Kill),
            _ => Err(LeftError::CommandNotFound),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Keybind {
    pub command: Command,
    pub value: Option<String>,
    pub modifier: Vec<String>,
    pub key: String,
    pub children: Option<Vec<Keybind>>,
}

// Needed as the kdl to_string functions add double quotes inside the string.
fn strip_quotes(mut string: String) -> String {
    string.retain(|c| c != '\"');
    string
}

impl TryFrom<&KdlNode> for Keybind {
    type Error = LeftError;

    fn try_from(node: &KdlNode) -> Result<Self> {
        let command: Command = Command::try_from(node.name.to_owned())?;
        let value: Option<String> = node.values.get(0).map(|val| strip_quotes(val.to_string()));
        let modifier_node: &KdlNode = node
            .children
            .iter()
            .find(|child| child.name == "modifier")
            .ok_or(LeftError::ModifierNotFound)?;
        let modifier: Vec<String> = modifier_node
            .values
            .iter()
            .map(|val| strip_quotes(val.to_string()))
            .collect();
        let key_node: &KdlNode = node
            .children
            .iter()
            .find(|child| child.name == "key")
            .ok_or(LeftError::KeyNotFound)?;
        let key: String = key_node
            .values
            .iter()
            .map(|val| strip_quotes(val.to_string()))
            .collect();
        let child_nodes: Vec<KdlNode> = node
            .children
            .iter()
            .filter(|child| Command::try_from(child.name.to_owned()).is_ok())
            .cloned()
            .collect();
        let children = if !child_nodes.is_empty() && command == Command::Chord {
            child_nodes
                .iter()
                .map(Keybind::try_from)
                .filter(Result::is_ok)
                .map(Result::ok)
                .collect()
        } else {
            None
        };
        Ok(Self {
            command,
            value,
            modifier,
            key,
            children,
        })
    }
}

pub fn load() -> Result<Vec<Keybind>> {
    let path = BaseDirectories::with_prefix("lefthk")?;
    fs::create_dir_all(&path.get_config_home())?;
    let file_name = path.place_config_file("config.kdl")?;
    if Path::new(&file_name).exists() {
        let contents = fs::read_to_string(file_name)?;
        let kdl = kdl::parse_document(contents)?;
        let mut keybinds = kdl
            .iter()
            .map(Keybind::try_from)
            .filter(Result::is_ok)
            .collect::<Result<Vec<Keybind>>>()?;
        if let Some(global_exit_chord) = keybinds
            .iter()
            .find(|kb| kb.command == Command::ExitChord)
            .cloned()
        {
            let chords = keybinds
                .iter_mut()
                .filter(|kb| kb.command == Command::Chord)
                .collect();
            propagate_exit_chord(chords, global_exit_chord);
        }

        return Ok(keybinds);
    }
    Err(LeftError::NoConfigFound)
}

fn propagate_exit_chord(chords: Vec<&mut Keybind>, exit_chord: Keybind) {
    for chord in chords {
        if let Some(children) = &mut chord.children {
            if !children.iter().any(|kb| kb.command == Command::ExitChord) {
                children.push(exit_chord.clone());
            }
            if let Some(parent_exit_chord) = children
                .iter()
                .find(|kb| kb.command == Command::ExitChord)
                .cloned()
            {
                let sub_chords = children
                    .iter_mut()
                    .filter(|kb| kb.command == Command::Chord)
                    .collect();
                propagate_exit_chord(sub_chords, parent_exit_chord);
            }
        }
    }
}

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
