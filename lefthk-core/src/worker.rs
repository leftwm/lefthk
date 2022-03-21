use crate::child::Children;
use crate::config::{self, Keybind};
use crate::errors::{self, Error, LeftError};
use crate::ipc::Pipe;
use crate::xkeysym_lookup;
use crate::xwrap::{self, XWrap};
#[cfg(feature = "watcher")]
use std::path::PathBuf;
use std::process::{Command, Stdio};
use x11_dl::xlib;
use xdg::BaseDirectories;

pub struct Worker {
    pub keybinds: Vec<Keybind>,
    #[cfg(feature = "watcher")]
    pub config_file: PathBuf,
    pub base_directory: BaseDirectories,
    pub xwrap: XWrap,
    pub children: Children,
    pub reload_requested: bool,
    pub kill_requested: bool,
    chord_keybinds: Option<Vec<Keybind>>,
    chord_elapsed: bool,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.xwrap.shutdown();
    }
}

impl Worker {
    #[cfg(feature = "watcher")]
    pub fn new(
        keybinds: Vec<Keybind>,
        config_file: PathBuf,
        base_directory: BaseDirectories,
    ) -> Self {
        Self {
            keybinds,
            config_file,
            base_directory,
            xwrap: XWrap::new(),
            children: Children::new(),
            reload_requested: false,
            kill_requested: false,
            chord_keybinds: None,
            chord_elapsed: false,
        }
    }

    #[cfg(feature = "watcher")]
    pub async fn event_loop(&mut self) {
        use crate::config::watcher::Watcher;

        self.xwrap.grab_keys(&self.keybinds);
        let mut watcher = errors::exit_on_error!(Watcher::new(&self.config_file));
        let pipe_name = Pipe::pipe_name();
        let pipe_file = errors::exit_on_error!(self.base_directory.place_runtime_file(pipe_name));
        let mut pipe = errors::exit_on_error!(Pipe::new(pipe_file).await);
        loop {
            if self.kill_requested || self.reload_requested {
                break;
            }

            if self.chord_elapsed {
                self.xwrap.grab_keys(&self.keybinds);
                self.chord_keybinds = None;
                self.chord_elapsed = false;
            }

            let task_notify = xwrap::wait_readable(self.xwrap.task_notify.clone());
            tokio::pin!(task_notify);

            tokio::select! {
                _ = self.children.wait_readable() => {
                    self.children.reap();
                    continue;
                }
                _ = &mut task_notify => {
                    let event_in_queue = self.xwrap.queue_len();
                    for _ in 0..event_in_queue {
                        let xlib_event = self.xwrap.get_next_event();
                        self.handle_event(&xlib_event);
                    }
                    continue;
                }
                _ = watcher.wait_readable(), if cfg!(watcher) => {
                    if watcher.has_events() {
                        errors::exit_on_error!(watcher.refresh_watch(&self.config_file));
                        self.reload_requested = true;
                    }
                    continue;
                }
                Some(command) = pipe.read_command() => {
                    match command {
                        config::Command::Reload => self.reload_requested = true,
                        config::Command::Kill => self.kill_requested = true,
                        _ => (),
                    }
                    continue;
                }
            }
        }
    }

    #[cfg(not(feature = "watcher"))]
    pub fn new(keybinds: Vec<Keybind>, base_directory: BaseDirectories) -> Self {
        Self {
            keybinds,
            base_directory,
            xwrap: XWrap::new(),
            children: Default::default(),
            reload_requested: false,
            kill_requested: false,
            chord_keybinds: None,
            chord_elapsed: false,
        }
    }

    #[cfg(not(feature = "watcher"))]
    pub async fn event_loop(&mut self) {
        self.xwrap.grab_keys(&self.keybinds);
        let pipe_name = Pipe::pipe_name();
        let pipe_file = errors::exit_on_error!(self.base_directory.place_runtime_file(pipe_name));
        let mut pipe = errors::exit_on_error!(Pipe::new(pipe_file).await);
        loop {
            if self.kill_requested || self.reload_requested {
                break;
            }

            if self.chord_elapsed {
                self.xwrap.grab_keys(&self.keybinds);
                self.chord_keybinds = None;
                self.chord_elapsed = false;
            }

            let task_notify = xwrap::wait_readable(self.xwrap.task_notify.clone());
            tokio::pin!(task_notify);

            tokio::select! {
                _ = self.children.wait_readable() => {
                    self.children.reap();
                    continue;
                }
                _ = &mut task_notify => {
                    let event_in_queue = self.xwrap.queue_len();
                    for _ in 0..event_in_queue {
                        let xlib_event = self.xwrap.get_next_event();
                        self.handle_event(&xlib_event);
                    }
                    continue;
                }
                Some(command) = pipe.read_command() => {
                    match command {
                        config::Command::Reload => self.reload_requested = true,
                        config::Command::Kill => self.kill_requested = true,
                        _ => (),
                    }
                    continue;
                }
            }
        }
    }

    fn handle_event(&mut self, xlib_event: &xlib::XEvent) {
        let error = match xlib_event.get_type() {
            xlib::KeyPress => self.key_press(&xlib::XKeyEvent::from(xlib_event)),
            xlib::MappingNotify => self.mapping_notify(&mut xlib::XMappingEvent::from(xlib_event)),
            _ => return,
        };
        let _ = errors::log_on_error!(error);
    }

    fn key_press(&mut self, event: &xlib::XKeyEvent) -> Error {
        let key = self.xwrap.keycode_to_keysym(event.keycode);
        let mask = xkeysym_lookup::clean_mask(event.state);
        if let Some(keybind) = self.get_keybind((mask, key)) {
            match keybind.command {
                config::Command::Chord(children) => {
                    self.chord_keybinds = Some(children);
                    if let Some(keybinds) = &self.chord_keybinds {
                        self.xwrap.grab_keys(keybinds);
                    }
                }
                config::Command::Execute(value) => {
                    self.chord_elapsed = self.chord_keybinds.is_some();
                    return self.exec(&value);
                }
                config::Command::ExitChord => {
                    if self.chord_keybinds.is_some() {
                        self.chord_elapsed = true;
                    }
                }
                config::Command::Reload => self.reload_requested = true,
                config::Command::Kill => self.kill_requested = true,
            }
        } else {
            return Err(LeftError::CommandNotFound);
        }
        Ok(())
    }

    fn get_keybind(&self, mask_key_pair: (u32, u32)) -> Option<Keybind> {
        let keybinds = if let Some(keybinds) = &self.chord_keybinds {
            keybinds
        } else {
            &self.keybinds
        };
        keybinds
            .iter()
            .find(|keybind| {
                if let Some(key) = xkeysym_lookup::into_keysym(&keybind.key) {
                    let mask = xkeysym_lookup::into_modmask(&keybind.modifier);
                    return mask_key_pair == (mask, key);
                }
                false
            })
            .cloned()
    }

    fn mapping_notify(&self, event: &mut xlib::XMappingEvent) -> Error {
        if event.request == xlib::MappingModifier || event.request == xlib::MappingKeyboard {
            return self.xwrap.refresh_keyboard(event);
        }
        Ok(())
    }

    /// Sends command for execution
    /// Assumes STDIN/STDOUT unwanted.
    pub fn exec(&mut self, command: &str) -> Error {
        let child = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?;
        self.children.insert(child);
        Ok(())
    }
}
