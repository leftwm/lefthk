use crate::config::{self, Keybind, Watcher};
use crate::errors::{self, Error, LeftError};
use crate::ipc::Pipe;
use crate::xwrap::XWrap;
use crate::{xkeysym_lookup, xwrap};
use std::process::{Command, Stdio};
use x11_dl::xlib;
use xdg::BaseDirectories;

pub struct Worker {
    pub keybinds: Vec<Keybind>,
    pub xwrap: XWrap,
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
    pub fn new(keybinds: Vec<Keybind>) -> Self {
        Self {
            keybinds,
            xwrap: XWrap::new(),
            reload_requested: false,
            kill_requested: false,
            chord_keybinds: None,
            chord_elapsed: false,
        }
    }

    pub async fn event_loop(&mut self) {
        self.xwrap.grab_keys(&self.keybinds);
        let path = errors::exit_on_error!(BaseDirectories::with_prefix("lefthk"));
        let config_file = errors::exit_on_error!(path.place_config_file("config.kdl"));
        let mut watcher = errors::exit_on_error!(Watcher::new(&config_file));
        let pipe_file = errors::exit_on_error!(path.place_runtime_file("commands.pipe"));
        let mut pipe = errors::exit_on_error!(Pipe::new(pipe_file).await);
        loop {
            if self.kill_requested {
                break;
            }

            if self.reload_requested {
                match config::load() {
                    Ok(keybinds) => {
                        if self.keybinds != keybinds {
                            self.keybinds = keybinds;
                            self.xwrap.grab_keys(&self.keybinds);
                        }
                    }
                    Err(err) => log::error!("Unable to load new config due to error: {}", err),
                }
                self.reload_requested = false;
                continue;
            }

            if self.chord_elapsed {
                self.xwrap.grab_keys(&self.keybinds);
                self.chord_keybinds = None;
                self.chord_elapsed = false;
            }

            let task_notify = xwrap::wait_readable(self.xwrap.task_notify.clone());
            tokio::pin!(task_notify);

            tokio::select! {
                _ = &mut task_notify => {
                    let event_in_queue = self.xwrap.queue_len();
                    for _ in 0..event_in_queue {
                        let xlib_event = self.xwrap.get_next_event();
                        self.handle_event(&xlib_event);
                    }
                    continue;
                }
                _ = watcher.wait_readable() => {
                    if watcher.has_events() {
                        errors::exit_on_error!(watcher.refresh_watch(&config_file));
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
                config::Command::Chord => {
                    self.chord_keybinds = keybind.children;
                    if let Some(keybinds) = &self.chord_keybinds {
                        self.xwrap.grab_keys(keybinds);
                    }
                }
                config::Command::Execute => {
                    self.chord_elapsed = self.chord_keybinds.is_some();
                    if let Some(value) = &keybind.value {
                        return exec(value);
                    }
                    return Err(LeftError::ValueNotFound);
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
}

/// Sends command for execution
/// Assumes STDIN/STDOUT unwanted.
pub fn exec(command: &str) -> Error {
    Command::new("sh")
        .arg("-c")
        .arg(&command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()?;
    Ok(())
}
