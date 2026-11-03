use crate::errors::{LeftError, Result};
use lefthk_core::config::{
    Command as core_command, Keybind as core_keybind, command as command_mod,
};
use serde::{Deserialize, Serialize};

use crate::config::{command::Command, key::Key};

// TODO: Replace with expr_2024
macro_rules! get_key {
    ($expr:expr_2021 $(,)?) => {
        match $expr {
            Key::Key(key) => key,
            Key::Keys(_) => return Err(LeftError::SingleKeyNeeded),
        }
    };
}

// TODO: Replace with expr_2024
macro_rules! get_keys {
    ($expr:expr_2021 $(,)?) => {
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
    pub modifier: Option<Vec<String>>,
    pub key: Key,
    pub on_release: Option<bool>,
}

pub(crate) fn try_from(kb: Keybind, default_modifier: &[String]) -> Result<Vec<core_keybind>> {
    let on_release = kb.on_release.unwrap_or(false);
    let command_key_pairs: Vec<(Box<dyn core_command>, String, bool)> = match kb.command {
        Command::Chord(children) if !children.is_empty() => {
            let key = get_key!(kb.key);
            let children = children
                .iter()
                .filter_map(|kb| match try_from(kb.clone(), default_modifier) {
                    Ok(keybinds) => Some::<Vec<lefthk_core::config::Keybind>>(keybinds),
                    Err(err) => {
                        tracing::error!("Invalid key binding: {}\n{:?}", err, kb);
                        None
                    }
                })
                .flatten()
                .collect::<Vec<lefthk_core::config::Keybind>>();

            vec![(Box::new(command_mod::Chord::new(children)), key, on_release)]
        }
        Command::Chord(_) => return Err(LeftError::ChildrenNotFound),
        Command::Execute(value) if !value.is_empty() => {
            let keys = get_key!(kb.key);
            vec![(
                (Box::new(command_mod::Execute::new(&value))),
                keys,
                on_release,
            )]
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
                .map(|(i, v)| {
                    (
                        Box::new(command_mod::Execute::new(&v)) as Box<dyn core_command>,
                        keys[i].clone(),
                        on_release,
                    )
                })
                .collect()
        }
        Command::Executes(_) => return Err(LeftError::ValuesNotFound),
        Command::ExitChord => {
            let keys = get_key!(kb.key);
            vec![((Box::new(command_mod::ExitChord::new())), keys, on_release)]
        }
        Command::Reload => {
            let keys = get_key!(kb.key);
            vec![((Box::new(command_mod::Reload::new())), keys, on_release)]
        }
        Command::Kill => {
            let keys = get_key!(kb.key);
            vec![((Box::new(command_mod::Kill::new())), keys, on_release)]
        }
    };
    let keybinds = command_key_pairs
        .iter()
        .map(|(c, k, o)| core_keybind {
            command: c.normalize(),
            modifier: kb
                .modifier
                .clone()
                .unwrap_or_else(|| default_modifier.to_vec()),
            key: k.clone(),
            on_release: *o,
        })
        .collect();
    Ok(keybinds)
}
