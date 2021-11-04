# LeftHK
LeftHK - A hotkey daemon written in Rust

*THIS IS BETA SOFTWARE*

The configuration file should be created in ~/.config/lefthk/ and called config.kdl. If the configuration file is not created the program will exit.
Example config:
```kdl
Execute "st" {
    modifier "Mod4"
    key "x"
}

Kill {
    modifier "Mod4" "Shift"
    key "q"
}

Reload {
    modifier "Mod4"
    key "r"
}

ExitChord {
    modifier "Mod4"
    key "q"
}

Chord {
    modifier "Mod4"
    key "c"
    
    Execute "st -e htop" {
        modifier "Mod4"
        key "x"
    }

    Kill {
        modifier "Mod4"
        key "c"
    } 
}
```
Reload, Kill, Chord, and ExitChord are the only internal commands. To run a normal command you need 
to call Execute, with the added value of the command. A chord can accept any amount and type of extra
keybind nodes, which when started blocks previous keybinds and will exit once a sub-keybind is 
executed. A Chord will take the ExitChord set within it first, then if not set it will take the 
ExitChord from its parent (e.g. a Chord within a Chord will take the ExitChord from the previous Chord). 
There is a pipe which receives commands through $XDG_RUNTIME_DIR/lefthk/commands.pipe, currently
only accepts Reload and Kill.

If the config file changes it will live update.
