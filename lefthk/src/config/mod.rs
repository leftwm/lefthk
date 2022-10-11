pub mod command;
pub mod keybind;
pub mod key;

use crate::errors::{LeftError, Result};

use std::{convert::TryFrom, fs, path::Path};
use xdg::BaseDirectories;

use self::{command::Command, keybind::Keybind};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Config {
    keybinds: Vec<Keybind>,
}

impl lefthk_core::config::Config for Config {
    fn mapped_bindings(&self) -> Vec<lefthk_core::config::Keybind> {
        self.keybinds
            .iter()
            .filter_map(|kb| match Keybind::try_from(kb.clone()) {
                Ok(keybinds) => Some(keybinds),
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
