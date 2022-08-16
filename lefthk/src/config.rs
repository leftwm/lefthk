use crate::errors::{LeftError, Result};
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fs, path::Path};
use xdg::BaseDirectories;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum Command {
    Chord(Vec<Keybind>),
    Execute(String),
    ExitChord,
    Reload,
    Kill,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Eq)]
pub struct Keybind {
    pub command: Command,
    pub modifier: Vec<String>,
    pub key: String,
}

impl TryFrom<Keybind> for lefthk_core::config::Keybind {
    type Error = LeftError;

    fn try_from(kb: Keybind) -> Result<Self> {
        let command = match kb.command {
            Command::Chord(children) => {
                let children = children
                    .iter()
                    .filter_map(|kb| match TryFrom::try_from(kb.clone()) {
                        Ok(keybind) => Some(keybind),
                        Err(err) => {
                            log::error!("Invalid key binding: {}\n{:?}", err, kb);
                            None
                        }
                    })
                    .collect();

                lefthk_core::config::Command::Chord(children)
            }
            Command::Execute(value) => lefthk_core::config::Command::Execute(value),
            Command::ExitChord => lefthk_core::config::Command::ExitChord,
            Command::Reload => lefthk_core::config::Command::Reload,
            Command::Kill => lefthk_core::config::Command::Kill,
        };
        Ok(Self {
            command,
            modifier: kb.modifier,
            key: kb.key,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Config {
    keybinds: Vec<Keybind>,
}

impl lefthk_core::config::Config for Config {
    fn mapped_bindings(&self) -> Vec<lefthk_core::config::Keybind> {
        self.keybinds
            .iter()
            .filter_map(|kb| match TryFrom::try_from(kb.clone()) {
                Ok(keybind) => Some(keybind),
                Err(err) => {
                    log::error!("Invalid key binding: {}\n{:?}", err, kb);
                    None
                }
            })
            .collect()
    }
}

pub fn load() -> Result<Config> {
    let path = BaseDirectories::with_prefix("lefthk")?;
    fs::create_dir_all(&path.get_config_home())?;
    let file_name = path.place_config_file("config.ron")?;
    if Path::new(&file_name).exists() {
        let contents = fs::read_to_string(file_name)?;
        let mut config: Config = ron::from_str(&contents).expect("Ron error");
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
        println!("{:?}", config);

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
