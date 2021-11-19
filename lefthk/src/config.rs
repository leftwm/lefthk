use crate::errors::{LeftError, Result};
use kdl::KdlNode;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fs, path::Path};
use xdg::BaseDirectories;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Command {
    Chord,
    Execute,
    ExitChord,
    Reload,
    Kill,
}

impl TryFrom<&str> for Command {
    type Error = LeftError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "Chord" => Ok(Self::Chord),
            "Execute" => Ok(Self::Execute),
            "ExitChord" => Ok(Self::ExitChord),
            "Reload" => Ok(Self::Reload),
            "Kill" => Ok(Self::Kill),
            _ => Err(LeftError::CommandNotFound),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Keybind {
    pub command: Command,
    pub value: Option<String>,
    pub modifier: Vec<String>,
    pub key: String,
    pub children: Option<Vec<Keybind>>,
}

impl TryFrom<Keybind> for lefthk_core::config::Keybind {
    type Error = LeftError;

    fn try_from(kb: Keybind) -> Result<Self> {
        let command = match kb.command {
            Command::Chord => {
                let children = kb
                    .children
                    .as_ref()
                    .ok_or(LeftError::ValueNotFound)?
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
            Command::Execute => lefthk_core::config::Command::Execute(
                kb.value
                    .as_ref()
                    .ok_or(LeftError::ValueNotFound)?
                    .to_owned(),
            ),
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

// Needed as the kdl to_string functions add double quotes inside the string.
fn strip_quotes(mut string: String) -> String {
    string.retain(|c| c != '\"');
    string
}

impl TryFrom<&KdlNode> for Keybind {
    type Error = LeftError;

    fn try_from(node: &KdlNode) -> Result<Self> {
        let command: Command = Command::try_from(&*node.name)?;
        let value: Option<String> = node.values.get(0).map(|val| strip_quotes(val.to_string()));
        let modifier_node: &KdlNode = node
            .children
            .iter()
            .find(|child| child.name == "modifier")
            .ok_or(LeftError::ModifierNotFound)?;
        let modifier: Vec<String> = modifier_node
            .values
            .iter()
            .map(|val| strip_quotes(val.to_string()))
            .collect();
        let key_node: &KdlNode = node
            .children
            .iter()
            .find(|child| child.name == "key")
            .ok_or(LeftError::KeyNotFound)?;
        let key: String = key_node
            .values
            .iter()
            .map(|val| strip_quotes(val.to_string()))
            .collect();
        let child_nodes: Vec<KdlNode> = node
            .children
            .iter()
            .filter(|child| Command::try_from(&*child.name).is_ok())
            .cloned()
            .collect();
        let children = if !child_nodes.is_empty() && command == Command::Chord {
            child_nodes
                .iter()
                .map(Keybind::try_from)
                .filter(Result::is_ok)
                .map(Result::ok)
                .collect()
        } else {
            None
        };
        Ok(Self {
            command,
            value,
            modifier,
            key,
            children,
        })
    }
}

pub fn load() -> Result<Config> {
    let path = BaseDirectories::with_prefix("lefthk")?;
    fs::create_dir_all(&path.get_config_home())?;
    let file_name = path.place_config_file("config.kdl")?;
    if Path::new(&file_name).exists() {
        let contents = fs::read_to_string(file_name)?;
        let kdl = kdl::parse_document(contents)?;
        let mut keybinds = kdl
            .iter()
            .map(Keybind::try_from)
            .filter(Result::is_ok)
            .collect::<Result<Vec<Keybind>>>()?;
        let global_exit_chord = keybinds
            .iter()
            .find(|kb| kb.command == Command::ExitChord)
            .cloned();
        let chords = keybinds
            .iter_mut()
            .filter(|kb| kb.command == Command::Chord)
            .collect();
        propagate_exit_chord(chords, global_exit_chord);

        return Ok(Config { keybinds });
    }
    Err(LeftError::NoConfigFound)
}

fn propagate_exit_chord(chords: Vec<&mut Keybind>, exit_chord: Option<Keybind>) {
    for chord in chords {
        if let Some(children) = &mut chord.children {
            if !children.iter().any(|kb| kb.command == Command::ExitChord) {
                if let Some(ref exit_chord) = exit_chord {
                    children.push(exit_chord.clone());
                }
            }
            let parent_exit_chord = children
                .iter()
                .find(|kb| kb.command == Command::ExitChord)
                .cloned();
            let sub_chords = children
                .iter_mut()
                .filter(|kb| kb.command == Command::Chord)
                .collect();
            propagate_exit_chord(sub_chords, parent_exit_chord);
        }
    }
}
