# LeftHK
LeftHK - A hotkey daemon written in Rust

*THIS IS BETA SOFTWARE*

The configuration file should be created in ~/.config/lefthk/ and called config.ron. If the configuration file is not created the program will exit.
Example config:
```ron
Config(
    default_modifier: ["Mod4", "Shift"],
    keybinds: [
        Keybind(
            command: Execute("st -e htop"),
            key: Key("x"),
        ),
        Keybind(
            command: Executes(["st -e htop", "st -e bpytop"]),
            key: Keys(["x", "m"]),
        ),
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
    ]
)
```
Reload, Kill, Chord, and ExitChord are the only internal commands. To run a normal command you need 
to call Execute or Executes, with the added value or values of the command. A chord can accept any amount and type of extra
keybinds, which when started blocks previous keybinds and will exit once a sub-keybind is 
executed. A Chord will take the ExitChord set within it first, then if not set it will take the 
ExitChord from its parent (e.g. a Chord within a Chord will take the ExitChord from the previous Chord). 
There is a pipe which receives commands through $XDG_RUNTIME_DIR/lefthk/commands.pipe, currently
only accepts Reload and Kill.
