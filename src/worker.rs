use crate::config::{self, Keybind, Watcher};
use crate::errors::{Error, LeftError};
use crate::ipc::Pipe;
use crate::xkeysym_lookup;
use crate::xwrap::XWrap;
use std::process::{Command, Stdio};
use x11_dl::xlib;

pub struct Worker {
    pub keybinds: Vec<Keybind>,
    pub xwrap: XWrap,
    pub watcher: Watcher,
    pub reload_requested: bool,
    pub kill_requested: bool,
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
            watcher: Watcher::new(),
            reload_requested: false,
            kill_requested: false,
        }
    }

    pub async fn event_loop(&mut self) {
        self.xwrap.grab_keys(&self.keybinds);
        let mut pipe = Pipe::new()
            .await
            .expect("ERROR: Could not connect to pipe.");
        loop {
            if self.kill_requested || self.reload_requested {
                break;
            }

            tokio::select! {
                _ = self.xwrap.wait_readable(), if !self.reload_requested => {
                    let event_in_queue = self.xwrap.queue_len();
                    for _ in 0..event_in_queue {
                        let xlib_event = self.xwrap.get_next_event();
                        self.handle_event(&xlib_event);
                    }
                    continue;
                }
                _ = self.watcher.wait_readable(), if !self.reload_requested => {
                    if self.watcher.has_events() {
                        self.reload_requested = true;
                    }
                    continue;
                }
                Some(command) = pipe.read_command(), if !self.reload_requested => {
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
        let _ = error.map_err(|err| log::error!("{}", err));
    }

    fn key_press(&mut self, event: &xlib::XKeyEvent) -> Error {
        let key = self.xwrap.keycode_to_keysym(event.keycode);
        let mask = xkeysym_lookup::clean_mask(event.state);
        if let Some(keybind) = self.get_keybind((mask, key)) {
            match keybind.command {
                config::Command::Execute => {
                    if let Some(value) = &keybind.value {
                        return exec(value);
                    }
                    return Err(LeftError::ValueNotFound);
                }
                config::Command::Reload => self.reload_requested = true,
                config::Command::Kill => self.kill_requested = true,
            }
        } else {
            return Err(LeftError::CommandNotFound);
        }
        Ok(())
    }

    fn get_keybind(&self, mask_key_pair: (u32, u32)) -> Option<&Keybind> {
        self.keybinds.iter().find(|keybind| {
            if let Some(key) = xkeysym_lookup::into_keysym(&keybind.key) {
                let mask = xkeysym_lookup::into_modmask(&keybind.modifier);
                return mask_key_pair == (mask, key);
            }
            false
        })
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
