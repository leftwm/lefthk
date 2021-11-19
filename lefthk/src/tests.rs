/// Config Testing
#[cfg(test)]
mod config {
    use crate::{
        config::{Command, Keybind},
        errors::Result,
    };
    use kdl::{KdlNode, KdlValue};
    use std::convert::TryFrom;

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
        let exit_chord: KdlNode = KdlNode {
            name: "ExitChord".to_owned(),
            children: vec![modifier.clone(), key.clone()],
            ..KdlNode::default()
        };
        let chord: KdlNode = KdlNode {
            name: "Chord".to_owned(),
            children: vec![modifier, key, execute.clone(), reload.clone(), kill.clone()],
            ..KdlNode::default()
        };

        let nodes: Vec<KdlNode> = vec![chord, execute, exit_chord, reload, kill];
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
        let exit_chord_kb: Keybind = Keybind {
            command: Command::ExitChord,
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
        let keybinds: Vec<Keybind> = vec![chord_kb, execute_kb, exit_chord_kb, reload_kb, kill_kb];
        assert_eq!(parsed_keybands, keybinds);
    }
}
