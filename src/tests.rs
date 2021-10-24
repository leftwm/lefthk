/// Test Helpers
#[cfg(test)]
pub(crate) mod test {
    pub async fn temp_path() -> std::io::Result<std::path::PathBuf> {
        tokio::task::spawn_blocking(|| tempfile::Builder::new().tempfile_in("./target"))
            .await
            .expect("Blocking task joined")?
            .into_temp_path()
            .keep()
            .map_err(Into::into)
    }
}

/// Config Testing
#[cfg(test)]
mod config {
    use std::convert::TryFrom;

    use kdl::{KdlNode, KdlValue};
    use tokio::{fs, io::AsyncWriteExt};

    use crate::{
        config::{Command, Keybind, Watcher},
        errors::Result,
    };

    use super::test::temp_path;

    #[test]
    fn parse_kdl_nodes() {
        let modifier: KdlNode = KdlNode {
            name: "modifier".to_owned(),
            values: vec![KdlValue::String("Mod4".to_owned())],
            ..KdlNode::default()
        };
        let key: KdlNode = KdlNode {
            name: "key".to_owned(),
            values: vec![KdlValue::String("x".to_owned())],
            ..KdlNode::default()
        };
        let execute: KdlNode = KdlNode {
            name: "Execute".to_owned(),
            values: vec![KdlValue::String("st".to_owned())],
            children: vec![modifier.clone(), key.clone()],
            ..KdlNode::default()
        };
        let reload: KdlNode = KdlNode {
            name: "Reload".to_owned(),
            children: vec![modifier.clone(), key.clone()],
            ..KdlNode::default()
        };
        let kill: KdlNode = KdlNode {
            name: "Kill".to_owned(),
            children: vec![modifier.clone(), key.clone()],
            ..KdlNode::default()
        };
        let chord: KdlNode = KdlNode {
            name: "Chord".to_owned(),
            children: vec![
                modifier.clone(),
                key.clone(),
                execute.clone(),
                reload.clone(),
                kill.clone(),
            ],
            ..KdlNode::default()
        };
        let nodes: Vec<KdlNode> = vec![chord, execute, reload, kill];
        let parsed_keybands: Vec<Keybind> = nodes
            .iter()
            .map(Keybind::try_from)
            .filter(Result::is_ok)
            .collect::<Result<Vec<Keybind>>>()
            .expect("Failed to parse nodes.");

        let execute_kb: Keybind = Keybind {
            command: Command::Execute,
            value: Some("st".to_owned()),
            modifier: vec!["Mod4".to_owned()],
            key: "x".to_owned(),
            children: None,
        };
        let reload_kb: Keybind = Keybind {
            command: Command::Reload,
            value: None,
            modifier: vec!["Mod4".to_owned()],
            key: "x".to_owned(),
            children: None,
        };
        let kill_kb: Keybind = Keybind {
            command: Command::Kill,
            value: None,
            modifier: vec!["Mod4".to_owned()],
            key: "x".to_owned(),
            children: None,
        };
        let chord_kb: Keybind = Keybind {
            command: Command::Chord,
            value: None,
            modifier: vec!["Mod4".to_owned()],
            key: "x".to_owned(),
            children: Some(vec![execute_kb.clone(), reload_kb.clone(), kill_kb.clone()]),
        };
        let keybinds: Vec<Keybind> = vec![chord_kb, execute_kb, reload_kb, kill_kb];
        assert_eq!(parsed_keybands, keybinds);
    }

    #[tokio::test]
    async fn check_watcher() {
        let config_file = temp_path().await.unwrap();
        let watcher = Watcher::new(config_file.clone());

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&config_file)
            .await
            .unwrap();
        file.write_all(b"\n").await.unwrap();
        file.flush().await.unwrap();

        assert!(true, "{}", watcher.has_events());
    }
}

/// IPC Testing
#[cfg(test)]
mod ipc {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    use crate::config::Command;
    use crate::ipc::Pipe;

    use super::test::temp_path;

    #[tokio::test]
    async fn read_command() {
        let pipe_file = temp_path().await.unwrap();
        let mut command_pipe = Pipe::new(pipe_file.clone()).await.unwrap();

        let mut pipe = fs::OpenOptions::new()
            .write(true)
            .open(&pipe_file)
            .await
            .unwrap();
        pipe.write_all(b"Reload\n").await.unwrap();
        pipe.flush().await.unwrap();

        assert_eq!(Command::Reload, command_pipe.read_command().await.unwrap());
    }
}
