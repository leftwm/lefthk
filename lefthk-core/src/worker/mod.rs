pub mod context;

use crate::child::Children;
use crate::config::{command, Keybind};
use crate::errors::{self, LeftError};
use crate::evdev::EvDev;
use crate::ipc::Pipe;
use crate::keysym_lookup;
use evdev_rs::enums::{EventCode, EV_KEY};
use evdev_rs::InputEvent;
use xdg::BaseDirectories;

#[derive(Clone, Copy, Debug)]
enum KeyEventType {
    Release,
    Press,
    Repeat,
    Unknown,
}

impl From<i32> for KeyEventType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Release,
            1 => Self::Press,
            2 => Self::Repeat,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Status {
    Reload,
    Kill,
    Continue,
}

pub struct Worker {
    keybinds: Vec<Keybind>,
    base_directory: BaseDirectories,

    keys_pressed: Vec<EV_KEY>,

    pub evdev: EvDev,
    pub children: Children,
    pub status: Status,

    /// "Chord Context": Holds the relevant data for chording
    pub chord_ctx: context::Chord,
}

impl Worker {
    pub fn new(keybinds: Vec<Keybind>, base_directory: BaseDirectories) -> Self {
        Self {
            status: Status::Continue,
            keybinds,
            base_directory,
            keys_pressed: Vec::new(),
            evdev: EvDev::new(),
            children: Children::default(),
            chord_ctx: context::Chord::new(),
        }
    }

    pub async fn event_loop(mut self) -> Status {
        let mut pipe = self.get_pipe().await;

        while self.status == Status::Continue {
            self.evaluate_chord();

            tokio::select! {
                () = self.children.wait_readable() => {
                    println!("Reaping children");
                    self.children.reap();
                }
                Some(event) = self.evdev.task_receiver.recv() => {
                    self.handle_event(&event);
                }
                Some(command) = pipe.get_next_command() => {
                    errors::log_on_error!(command.execute(&mut self));
                }
            };
        }

        self.status
    }

    async fn get_pipe(&self) -> Pipe {
        let pipe_name = Pipe::pipe_name();
        let pipe_file = errors::exit_on_error!(self.base_directory.place_runtime_file(pipe_name));
        errors::exit_on_error!(Pipe::new(pipe_file).await)
    }

    fn handle_event(&mut self, event: &InputEvent) {
        let r#type = KeyEventType::from(event.value);
        match r#type {
            KeyEventType::Release => {
                if let EventCode::EV_KEY(key) = event.event_code {
                    if let Some(index) = self.keys_pressed.iter().position(|&k| k == key) {
                        self.keys_pressed.remove(index);
                    }
                }
            }
            KeyEventType::Press => {
                let mut new_key = false;
                if let EventCode::EV_KEY(key) = event.event_code {
                    if !self.keys_pressed.contains(&key) {
                        self.keys_pressed.push(key);
                        new_key = true;
                    }
                }
                if new_key {
                    println!("Keys: {:?}", self.keys_pressed);
                    if let Some(keybind) = self.check_for_keybind() {
                        if let Ok(command) = command::denormalize(&keybind.command) {
                            let _ = command.execute(self);
                        } else {
                            errors::log_on_error!(Err(LeftError::CommandNotFound));
                        }
                    }
                }
            }
            KeyEventType::Repeat => {}
            KeyEventType::Unknown => {}
        }
    }

    fn check_for_keybind(&self) -> Option<Keybind> {
        let keybinds = if let Some(keybinds) = &self.chord_ctx.keybinds {
            keybinds
        } else {
            &self.keybinds
        };
        keybinds
            .iter()
            .find(|keybind| {
                if let Some(key) = keysym_lookup::into_key(&keybind.key) {
                    let modifiers = keysym_lookup::into_keys(&keybind.modifier);
                    let keys: Vec<EV_KEY> =
                        modifiers.into_iter().chain(std::iter::once(key)).collect();
                    return keys.iter().all(|key| self.keys_pressed.contains(key));
                }
                false
            })
            .cloned()
    }
    //
    // fn handle_mapping_notify(&self, event: &mut xlib::XMappingEvent) -> Error {
    //     if event.request == xlib::MappingModifier || event.request == xlib::MappingKeyboard {
    //         return self.xwrap.refresh_keyboard(event);
    //     }
    //     Ok(())
    // }
}
