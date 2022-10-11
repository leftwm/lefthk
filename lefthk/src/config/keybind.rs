use crate::errors::{LeftError, Result};
use lefthk_core::config::Command as core_command;
use lefthk_core::config::Keybind as core_keybind;
use lefthk_core::config::command as command_mod;
use serde::Deserialize;
use serde::Serialize;

use super::{command::Command, key::Key};

use std::convert::TryFrom;

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

pub type Keybinds = Vec<Keybind>;

#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize)]
pub struct Keybind {
    pub command: Command,
    pub modifier: Vec<String>,
    pub key: Key,
}

impl TryFrom<Keybind> for Vec<core_keybind> {
    type Error = LeftError;

    fn try_from(kb: Keybind) -> Result<Self> {
        let command_key_pairs: Vec<(Box<dyn core_command>, String)> = match kb.command {
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

                vec![(Box::new(command_mod::Chord::new(children)), key)]
            }
            Command::Chord(_) => return Err(LeftError::ChildrenNotFound),
            Command::Execute(value) if !value.is_empty() => {
                let keys = get_key!(kb.key);
                vec![(Box::new(command_mod::Execute::new(value)), keys)]
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
                    .map(|(i, v)| (Box::new(command_mod::Execute::new(v.to_owned())) as Box<dyn core_command>, keys[i].clone()))
                    .collect()
            }
            Command::Executes(_) => return Err(LeftError::ValuesNotFound),
            Command::ExitChord => {
                let keys = get_key!(kb.key);
                vec![(Box::new(command_mod::ExitChord::new()), keys)]
            }
            Command::Reload => {
                let keys = get_key!(kb.key);
                vec![(Box::new(command_mod::Reload::new()), keys)]
            }
            Command::Kill => {
                let keys = get_key!(kb.key);
                vec![(Box::new(command_mod::Kill::new()), keys)]
            }
        };
        let keybinds = command_key_pairs
            .iter()
            .map(|(c, k)| core_keybind {
                command: c.normalize(),
                modifier: kb.modifier.clone(),
                key: k.to_owned(),
            })
            .collect();
        Ok(keybinds)
    }
}

