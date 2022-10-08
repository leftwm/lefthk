use crate::errors::{LeftError, Result};

use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fs, path::Path};
use xdg::BaseDirectories;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum Command {
    Chord(Vec<Keybind>),
    Execute(String),
    Executes(Vec<String>),
    ExitChord,
    Reload,
    Kill,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum Key {
    Key(String),
    Keys(Vec<String>),
}

macro_rules! get_key {
    ($expr:expr $(,)?) => {
        match $expr {
            Key::Key(key) => key,
            Key::Keys(_) => return Err(LeftError::SingleKeyNeeded),
        }
    };
}

macro_rules! get_keys {
    ($expr:expr $(,)?) => {
        match $expr {
            Key::Key(_) => return Err(LeftError::MultipleKeysNeeded),
            Key::Keys(keys) => keys,
        }
    };
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Eq)]
pub struct Keybind {
    pub command: Command,
    pub modifier: Vec<String>,
    pub key: Key,
}

impl TryFrom<Keybind> for Vec<Keybind> {
    type Error = LeftError;

    fn try_from(kb: Keybind) -> Result<Self> {
        let command_key_pairs: Vec<(Command, String)> = match kb.command {
            Command::Chord(children) if !children.is_empty() => {
                let key = get_key!(kb.key);
                let children = children
                    .iter()
                    .filter_map(|kb| match TryFrom::try_from(kb.clone()) {
                        Ok(keybinds) => Some::<Vec<lefthk_core::config::Keybind>>(keybinds),
                        Err(err) => {
                            tracing::error!("Invalid key binding: {}\n{:?}", err, kb);
                            None
                        }
                    })
                    .flatten()
                    .collect();

                vec![(Command::Chord(children), key)]
            }
            Command::Chord(_) => return Err(LeftError::ChildrenNotFound),
            Command::Execute(value) if !value.is_empty() => {
                let keys = get_key!(kb.key);
                vec![(Command::Execute(value), keys)]
            }
            Command::Execute(_) => return Err(LeftError::ValueNotFound),
            Command::Executes(values) if !values.is_empty() => {
                let keys = get_keys!(kb.key);
                if keys.len() != values.len() {
                    return Err(LeftError::NumberOfKeysDiffersFromValues);
                }
                values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (Command::Execute(v.to_owned()), keys[i].clone()))
                    .collect()
            }
            Command::Executes(_) => return Err(LeftError::ValuesNotFound),
            Command::ExitChord => {
                let keys = get_key!(kb.key);
                vec![(Command::ExitChord, keys)]
            }
            Command::Reload => {
                let keys = get_key!(kb.key);
                vec![(Command::Reload, keys)]
            }
            Command::Kill => {
                let keys = get_key!(kb.key);
                vec![(Command::Kill, keys)]
            }
        };
        let keybinds = command_key_pairs
            .iter()
            .map(|(c, k)| Keybind {
                command: c.clone(),
                modifier: kb.modifier.clone(),
                key: k.to_owned(),
            })
            .collect();
        Ok(keybinds)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Config {
    keybinds: Vec<Keybind>,
}

impl lefthk_core::config::Config for Config {
    fn mapped_bindings(&self) -> Vec<lefthk_core::config::Keybind> {
        self.keybinds
            .iter()
            .filter_map(|kb| match TryFrom::try_from(kb.clone()) {
                Ok(keybinds) => Some::<Vec<lefthk_core::config::Keybind>>(keybinds),
                Err(err) => {
                    tracing::error!("Invalid key binding: {}\n{:?}", err, kb);
                    None
                }
            })
            .flatten()
            .collect()
    }
}

pub fn load() -> Result<Config> {
    let path = BaseDirectories::with_prefix(lefthk_core::LEFTHK_DIR_NAME)?;
    fs::create_dir_all(&path.get_config_home())?;
    let file_name = path.place_config_file("config.ron")?;
    if Path::new(&file_name).exists() {
        let contents = fs::read_to_string(file_name)?;
        let mut config: Config = ron::from_str(&contents)?;
        let global_exit_chord = config
            .keybinds
            .iter()
            .find(|kb| matches!(kb.command, Command::ExitChord))
            .cloned();
        let chords: Vec<&mut Keybind> = config
            .keybinds
            .iter_mut()
            .filter(|kb| matches!(kb.command, Command::Chord(_)))
            .collect();
        propagate_exit_chord(chords, global_exit_chord);

        return Ok(config);
    }
    Err(LeftError::NoConfigFound)
}

fn propagate_exit_chord(chords: Vec<&mut Keybind>, exit_chord: Option<Keybind>) {
    for chord in chords {
        if let Command::Chord(children) = &mut chord.command {
            if !children.iter().any(|kb| kb.command == Command::ExitChord) {
                if let Some(ref exit_chord) = exit_chord {
                    children.push(exit_chord.clone());
                }
            }
            let parent_exit_chord = children
                .iter()
                .find(|kb| matches!(kb.command, Command::ExitChord))
                .cloned();
            let sub_chords = children
                .iter_mut()
                .filter(|kb| matches!(kb.command, Command::Chord(_)))
                .collect();
            propagate_exit_chord(sub_chords, parent_exit_chord);
        }
    }
}
