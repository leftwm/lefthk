use crate::child::Children;
use crate::config::{self, Keybind};
use crate::errors::{self, Error, LeftError};
use crate::ipc::Pipe;
use crate::xkeysym_lookup;
use crate::xwrap::XWrap;
use std::process::{Command, Stdio};
use x11_dl::xlib;
use xdg::BaseDirectories;

pub struct Worker {
    pub keybinds: Vec<Keybind>,
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
            println!("1");

            tokio::select! {
                _ = self.children.wait_readable() => {
                    self.children.reap();
                    continue;
                }
                _ = self.xwrap.wait_readable() => {
                    println!("2");
                    let event_in_queue = self.xwrap.queue_len();
                    for _ in 0..event_in_queue {
                        println!("3");
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
        errors::log_on_error!(error);
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
