pub mod context;

use std::path::PathBuf;

use crate::child::Children;
use crate::config::{command, Keybind};
use crate::errors::{self, LeftError};
use crate::evdev::{EvDev, Task};
use crate::ipc::Pipe;
use crate::keysym_lookup::{self, is_modifier, MOD_MASK};
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
    mods_pressed: Vec<MOD_MASK>,
    eaten_keys: Vec<EV_KEY>,
    eaten_mods: Vec<MOD_MASK>,

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
            mods_pressed: Vec::new(),
            eaten_keys: Vec::new(),
            eaten_mods: Vec::new(),
            evdev: EvDev::default(),
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
                Some(task) = self.evdev.task_receiver.recv() => {
                    match task {
                        Task::KeyboardEvent((path, event)) => {
                            self.handle_event(path, &event);
                        }
                        Task::KeyboardAdded(path) => {
                            self.evdev.add_device(path);
                        }
                        Task::KeyboardRemoved(path) => {
                            self.evdev.remove_device(path);
                        }
                    }
                }
                Some(command) = pipe.get_next_command() => {
                    errors::log!(command.execute(&mut self));
                }
            };
        }

        self.status
    }

    async fn get_pipe(&self) -> Pipe {
        let pipe_name = Pipe::pipe_name();
        let pipe_file = errors::exit!(self.base_directory.place_runtime_file(pipe_name));
        errors::exit!(Pipe::new(pipe_file).await)
    }

    fn handle_event(&mut self, path: PathBuf, event: &InputEvent) {
        let r#type = KeyEventType::from(event.value);
        let mut eaten = false;
        match r#type {
            KeyEventType::Release => {
                if let EventCode::EV_KEY(key) = event.event_code {
                    if is_modifier(&key) {
                        if let Ok(modifier) = key.try_into() {
                            self.mods_pressed.retain(|&m| m != modifier);
                            if self.eaten_mods.contains(&modifier) {
                                eaten = true;
                                self.eaten_mods.retain(|&m| m != modifier);
                            }
                        }
                    } else if let Some(index) = self.keys_pressed.iter().position(|&k| k == key) {
                        self.keys_pressed.remove(index);
                        if self.eaten_keys.contains(&key) {
                            eaten = true;
                            self.eaten_keys.retain(|&k| k != key);
                        }
                    }
                }
            }
            KeyEventType::Press => {
                let mut new_key = false;
                if let EventCode::EV_KEY(key) = event.event_code {
                    if is_modifier(&key) {
                        match key.try_into() {
                            Ok(modifier) if !self.mods_pressed.contains(&modifier) => {
                                self.mods_pressed.push(modifier);
                                new_key = true;
                            }
                            _ => {}
                        }
                    } else if !self.keys_pressed.contains(&key) {
                        self.keys_pressed.push(key);
                        new_key = true;
                    }
                }
                if new_key {
                    if let Some(keybind) = self.check_for_keybind() {
                        eaten = true;
                        self.keys_pressed
                            .iter()
                            .for_each(|&key| self.eaten_keys.push(key));
                        self.mods_pressed
                            .iter()
                            .for_each(|&key| self.eaten_mods.push(key));
                        if let Ok(command) = command::denormalize(&keybind.command) {
                            let _ = command.execute(self);
                        } else {
                            errors::log!(Err(LeftError::CommandNotFound));
                        }
                    }
                }
            }
            KeyEventType::Repeat | KeyEventType::Unknown => {}
        }
        if !eaten {
            self.pass_event(path, event);
        }
    }

    fn pass_event(&self, path: PathBuf, event: &InputEvent) {
        // println!("Passing event: {:?}", event);
        match self.evdev.devices.get(&path) {
            Some(device) => errors::log!(device.write_event(event)),
            None => errors::log!(Err(LeftError::UInputNotFound)),
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
                    let modifiers = keysym_lookup::into_mods(&keybind.modifier);
                    let matching = modifiers.is_empty() && self.mods_pressed.is_empty()
                        || (!self.mods_pressed.is_empty()
                            && self.mods_pressed.iter().all(|m| modifiers.contains(m)))
                            && (!modifiers.is_empty()
                                && modifiers.iter().all(|m| self.mods_pressed.contains(m)));
                    return self.keys_pressed == vec![key] && matching;
                }
                false
            })
            .cloned()
    }
}
