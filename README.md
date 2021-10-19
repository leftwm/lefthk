# LeftHK
LeftHK - A hotkey daemon written in Rust

*THIS IS A TEMPORARY REPOSITORY AND BETA SOFTWARE*

The configuartion file should be created in ~/.config/lefthk/ and called config.toml. If the configuration file is not created the program will exit.
Example config:
```toml
[[keybind]]
command = "Execute"
value = "st"
modifier = ["Mod4"]
key = "x"

[[keybind]]
command = "Kill"
modifier = ["Mod4", "Shift"]
key = "q"

[[keybind]]
command = "Reload"
modifier = ["Mod4"]
key = "r"
```
Reload and Kill are the only internal commands. To run a normal command you need to call Execute, with the added value of the command.
There is a pipe which receives commands through $XDG_RUNTIME_DIR/lefthk/commands.pipe, currently only accepts Reload and Kill.

If the config file changes it will live update ("reboots" but isn't noticable as it is so fast).
