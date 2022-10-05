/// Config Testing
#[cfg(test)]
mod config {
    use lefthk_core::config::{Command, Config, Keybind};

    use crate::config::Config as Cfg;

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
