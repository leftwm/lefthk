use crate::errors::{LeftError, Result};
use lefthk_core::config::Command as core_command;
use lefthk_core::config::Keybind as core_keybind;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
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
    pub modifier: Option<Vec<String>>,
    pub key: Key,
}

fn try_from(kb: Keybind, default_modifier: Vec<String>) -> Result<Vec<core_keybind>> {
    let command_key_pairs: Vec<(core_command, String)> = match kb.command {
        Command::Chord(children) if !children.is_empty() => {
            let key = get_key!(kb.key);
            let children = children
                .iter()
                .filter_map(|kb| match try_from(kb.clone(), default_modifier.clone()) {
                    Ok(keybinds) => Some::<Vec<lefthk_core::config::Keybind>>(keybinds),
                    Err(err) => {
                        tracing::error!("Invalid key binding: {}\n{:?}", err, kb);
                        None
                    }
                })
                .flatten()
                .collect();

            vec![(core_command::Chord(children), key)]
        }
        Command::Chord(_) => return Err(LeftError::ChildrenNotFound),
        Command::Execute(value) if !value.is_empty() => {
            let keys = get_key!(kb.key);
            vec![(core_command::Execute(value), keys)]
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
                .map(|(i, v)| (core_command::Execute(v.to_owned()), keys[i].clone()))
                .collect()
        }
        Command::Executes(_) => return Err(LeftError::ValuesNotFound),
        Command::ExitChord => {
            let keys = get_key!(kb.key);
            vec![(core_command::ExitChord, keys)]
        }
        Command::Reload => {
            let keys = get_key!(kb.key);
            vec![(core_command::Reload, keys)]
        }
        Command::Kill => {
            let keys = get_key!(kb.key);
            vec![(core_command::Kill, keys)]
        }
    };
    let keybinds = command_key_pairs
        .iter()
        .map(|(c, k)| core_keybind {
            command: c.clone(),
            modifier: kb.modifier.clone().unwrap_or(default_modifier.clone()),
            key: k.to_owned(),
        })
        .collect();
    Ok(keybinds)
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Config {
    default_modifier: Vec<String>,
    keybinds: Vec<Keybind>,
}

impl lefthk_core::config::Config for Config {
    fn mapped_bindings(&self) -> Vec<lefthk_core::config::Keybind> {
        self.keybinds
            .iter()
            .filter_map(
                |kb| match try_from(kb.clone(), self.default_modifier.clone()) {
                    Ok(keybinds) => Some::<Vec<lefthk_core::config::Keybind>>(keybinds),
                    Err(err) => {
                        tracing::error!("Invalid key binding: {}\n{:?}", err, kb);
                        None
                    }
                },
            )
            .flatten()
            .collect()
    }
}

impl Config {
    pub fn from_string(contents: String) -> Result<Self> {
        println!("{contents}");
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
}

pub fn load() -> Result<Config> {
    let path = BaseDirectories::with_prefix(lefthk_core::LEFTHK_DIR_NAME)?;
    fs::create_dir_all(&path.get_config_home())?;
    let file_name = path.place_config_file("config.ron")?;
    if Path::new(&file_name).exists() {
        let contents = fs::read_to_string(file_name)?;
        return Ok(Config::from_string(contents)?);
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

#[cfg(test)]
mod test {
    use lefthk_core::config::{Command, Config, Keybind};

    use super::Config as Cfg;

    #[test]
    fn parse_config() {
        let config = r#"#![enable(implicit_some)]
Config(
    default_modifier: ["Mod4", "Shift"],
    keybinds: [
        Keybind(
            command: Execute("st -e htop"),
            key: Key("x"),
        ),
        Keybind(
            command: Execute("st -e btm"),
            modifier: ["Mod4"],
            key: Key("c"),
        ),
    ]
)"#;
        let conf = Cfg::from_string(config.to_string());
        assert!(conf.is_ok());
        let conf = conf.unwrap();
        assert_eq!(conf.default_modifier.len(), 2);
        assert_eq!(
            conf.default_modifier,
            vec!["Mod4".to_string(), "Shift".to_string()]
        );
        let conf_mapped = conf.mapped_bindings();

        // Verify default modifier implementation
        let default_keybind = conf_mapped.first().unwrap();
        assert_eq!(default_keybind.modifier.len(), 2);
        assert_eq!(default_keybind.modifier, conf.default_modifier);

        // Verify own implementation
        let custom_keybind = conf_mapped.last().unwrap();
        assert_eq!(custom_keybind.modifier.len(), 1);
        assert_eq!(custom_keybind.modifier, vec!["Mod4".to_string()]);
    }

    #[test]
    fn parse_empty_config() {
        let config = r#"Config(
    default_modifier: ["Mod4", "Shift"],
    keybinds: []
)"#;
        let conf = Cfg::from_string(config.to_string());
        assert!(conf.is_ok());
        let conf = conf.unwrap();
        assert_eq!(conf.default_modifier.len(), 2);
        assert_eq!(
            conf.default_modifier,
            vec!["Mod4".to_string(), "Shift".to_string()]
        );
        let conf_mapped = conf.mapped_bindings();

        // Verify implementation
        assert_eq!(conf_mapped.len(), 0);
    }

    #[test]
    fn parse_none_config() {
        // Define empty string
        let conf = Cfg::from_string(String::new());
        assert!(conf.is_err());
    }

    #[test]
    fn parse_sub_keybind_config() {
        let config = r#"#![enable(implicit_some)]
Config(
    default_modifier: ["Mod4", "Shift"],
    keybinds: [
        Keybind(
            command: Chord([
                Keybind(
                    command: Execute("st -e htop"),
                    modifier: ["Mod4"],
                    key: Key("c"),
                ),
            ]),
            modifier: ["Mod4"],
            key: Key("c"),
        ),
        Keybind(
            command: Chord([
                Keybind(
                    command: Execute("st -e htop"),
                    key: Key("c"),
                ),
            ]),
            key: Key("c"),
        ),
    ]
)"#;
        let conf = Cfg::from_string(config.to_string());
        assert!(conf.is_ok());
        let conf = conf.unwrap();
        assert_eq!(conf.default_modifier.len(), 2);
        assert_eq!(
            conf.default_modifier,
            vec!["Mod4".to_string(), "Shift".to_string()]
        );
        let conf_mapped = conf.mapped_bindings();

        // Verify default modifier implementation
        let default_keybind = conf_mapped.last().unwrap();
        assert_eq!(default_keybind.modifier.len(), 2);
        assert_eq!(default_keybind.modifier, conf.default_modifier);
        assert_eq!(
            default_keybind.command,
            Command::Chord(vec![Keybind {
                command: Command::Execute("st -e htop".to_string()),
                modifier: vec!["Mod4".to_string(), "Shift".to_string()],
                key: "c".to_string(),
            }])
        );

        // Verify custom modifier implementation
        let custom_keybind = conf_mapped.first().unwrap();
        assert_eq!(custom_keybind.modifier.len(), 1);
        assert_eq!(custom_keybind.modifier, vec!["Mod4".to_string()]);
        assert_eq!(
            custom_keybind.command,
            Command::Chord(vec![Keybind {
                command: Command::Execute("st -e htop".to_string()),
                modifier: vec!["Mod4".to_string()],
                key: "c".to_string(),
            }])
        );
    }
}
