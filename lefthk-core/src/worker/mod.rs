pub mod context;

use crate::child::Children;
use crate::config::{Keybind, command};
use crate::errors::{self, Error, LeftError};
use crate::ipc::Pipe;
use crate::xkeysym_lookup;
use crate::xwrap::XWrap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use x11_dl::xlib;
use xdg::BaseDirectories;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Status {
    Reload,
    Kill,
    Continue,
}

#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize)]
pub struct StatefullKeybind {
    pub keybind: Keybind,
    already_pressed: bool,
}

impl StatefullKeybind {
    #[must_use]
    pub fn new(keybind: Keybind) -> Self {
        Self {
            keybind,
            already_pressed: false,
        }
    }
}

pub struct Worker {
    sf_keybinds: Vec<StatefullKeybind>,
    base_directory: BaseDirectories,

    pub xwrap: XWrap,
    pub children: Children,
    pub status: Status,

    /// "Chord Context": Holds the relevant data for chording
    pub chord_ctx: context::Chord,
}

impl Worker {
    #[must_use]
    pub fn new(keybinds: Vec<Keybind>, base_directory: BaseDirectories) -> Self {
        Self {
            status: Status::Continue,
            sf_keybinds: keybinds.into_iter().map(StatefullKeybind::new).collect(),
            base_directory,
            xwrap: XWrap::new(),
            children: Children::default(),
            chord_ctx: context::Chord::new(),
        }
    }

    pub async fn event_loop(mut self) -> Status {
        let detectable_autorepeat = self.xwrap.set_detectable_auto_repeat();
        self.xwrap.get_modifier_mapping();
        self.xwrap.grab_keys(&self.sf_keybinds);
        let mut pipe = self.get_pipe().await;

        while self.status == Status::Continue {
            self.xwrap.flush();

            self.evaluate_chord();

            tokio::select! {
                () = self.children.wait_readable() => {
                    self.children.reap();
                }
                () = self.xwrap.wait_readable() => {
                    let event_in_queue = self.xwrap.queue_len();
                    for _ in 0..event_in_queue {
                        let xlib_event = self.xwrap.get_next_event();
                        self.handle_event(&xlib_event, detectable_autorepeat);
                    }
                }
                Some(command) = pipe.get_next_command() => {
                    errors::log_on_error!(command.execute(&mut self));
                }
            };
        }

        self.status
    }

    async fn get_pipe(&self) -> Pipe {
        let pipe_name = Pipe::pipe_name();
        let pipe_file = errors::exit_on_error!(self.base_directory.place_runtime_file(pipe_name));
        errors::exit_on_error!(Pipe::new(pipe_file).await)
    }

    fn handle_event(&mut self, xlib_event: &xlib::XEvent, detectable_autorepeat: bool) {
        let error = match xlib_event.get_type() {
            xlib::KeyPress => {
                self.handle_key_press(&xlib::XKeyEvent::from(xlib_event), detectable_autorepeat)
            }
            xlib::KeyRelease => {
                self.handle_key_release(&xlib::XKeyEvent::from(xlib_event), detectable_autorepeat)
            }
            xlib::MappingNotify => {
                self.handle_mapping_notify(&mut xlib::XMappingEvent::from(xlib_event))
            }
            _ => return,
        };
        errors::log_on_error!(error);
    }

    fn handle_key_press(&mut self, event: &xlib::XKeyEvent, detectable_autorepeat: bool) -> Error {
        let key = self.xwrap.keycode_to_keysym(event.keycode)?;
        let mask = xkeysym_lookup::clean_mask(event.state);
        let matching_sf_keybinds = self.get_sf_keybind_pair_from_key_mod((mask, key));
        if let Some(sf_keybind) = matching_sf_keybinds.0 {
            if !sf_keybind.already_pressed {
                if let Some(companion_sf_keybind) = matching_sf_keybinds.1
                    && detectable_autorepeat
                {
                    sf_keybind.already_pressed = true;
                    companion_sf_keybind.already_pressed = true;
                }
                let result_command = command::denormalize(&sf_keybind.keybind.command);
                let command = result_command?;
                command.execute(self)?;
            }
        } else if let Some(sf_keybind) = matching_sf_keybinds.1
            && detectable_autorepeat
        {
            sf_keybind.already_pressed = true;
        }
        Ok(())
    }

    fn handle_key_release(
        &mut self,
        event: &xlib::XKeyEvent,
        detectable_autorepeat: bool,
    ) -> Error {
        if detectable_autorepeat {
            let key = self.xwrap.keycode_to_keysym(event.keycode)?;
            let mask = xkeysym_lookup::clean_mask(event.state);
            let mut commands = Vec::new();
            let sf_keybind_pair_list = self.get_sf_keybind_on_release((mask, key));
            // For each keybind, set already_pressed to false and build a list of command
            for sf_keybind_pair in sf_keybind_pair_list {
                if let Some(on_press_sf_keybind) = sf_keybind_pair.0 {
                    on_press_sf_keybind.already_pressed = false;
                }
                if let Some(on_release_sf_keybind) = sf_keybind_pair.1 {
                    on_release_sf_keybind.already_pressed = false;
                    if let Ok(command) =
                        command::denormalize(&on_release_sf_keybind.keybind.command)
                    {
                        commands.push(command);
                    }
                }
            } // release mut on keybinds
            // Execute all commands, and only once all got tried, return the first error or Ok.
            let command_result_list: Vec<_> = commands
                .iter()
                .map(|command| command.execute(self))
                .collect();
            for command_result in command_result_list {
                command_result?;
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Get keybind pair (on press/on release) for given key/mod combinaison. Will only return one pair .
    fn get_sf_keybind_pair_from_key_mod(
        &mut self,
        mask_key_pair: (u32, u32),
    ) -> (Option<&mut StatefullKeybind>, Option<&mut StatefullKeybind>) {
        let sf_keybinds = if let Some(sf_keybinds) = self.chord_ctx.sf_keybinds.as_mut() {
            sf_keybinds
        } else {
            &mut self.sf_keybinds
        };
        let mut matching_keybinds = (None, None);
        for sf_keybind in sf_keybinds.iter_mut() {
            if let Some(key) = xkeysym_lookup::into_keysym(&sf_keybind.keybind.key) {
                let mask = xkeysym_lookup::into_modmask(&sf_keybind.keybind.modifier);
                if mask_key_pair == (mask, key) {
                    if sf_keybind.keybind.on_release {
                        matching_keybinds.1 = Some(sf_keybind);
                    } else {
                        matching_keybinds.0 = Some(sf_keybind);
                    }
                }
            }
        }
        matching_keybinds
    }

    /// Get all keybinds that contains provided modifier OR key. Take modifier mask as parameter.
    fn get_sf_keybind_pair_list_from_key_mod(
        &mut self,
        optional_mask: Option<u32>,
        optional_key: Option<u32>,
    ) -> Vec<(Option<&mut StatefullKeybind>, Option<&mut StatefullKeybind>)> {
        let sf_keybinds = if let Some(sf_keybinds) = self.chord_ctx.sf_keybinds.as_mut() {
            sf_keybinds
        } else {
            &mut self.sf_keybinds
        };
        // define a hashmap to store matching items so we don't need to double loop
        let mut result = HashMap::new();
        for sf_keybind in sf_keybinds.iter_mut() {
            let mut is_match = false;
            if let Some(mask) = optional_mask {
                let keybind_mask = xkeysym_lookup::into_modmask(&sf_keybind.keybind.modifier);
                // consider keybind matches if its mask contains provided mask
                is_match = is_match || (mask & keybind_mask != 0);
            }
            if let Some(keybind_key) = xkeysym_lookup::into_keysym(&sf_keybind.keybind.key)
                && let Some(key) = optional_key
            {
                // consider keybind matches if its key matches provided key
                is_match = is_match || (key == keybind_key);
            }
            if is_match {
                // Store keybind in hashmap with entry name being key and mod concat
                // This way it's easier to add on_press/on_release counterpart
                let entry_name_mod_part = sf_keybind.keybind.modifier.join("-");
                let entry_name = format!("{entry_name_mod_part}{0}", sf_keybind.keybind.key);
                let entry = result.entry(entry_name).or_insert((None, None));
                if sf_keybind.keybind.on_release {
                    entry.1 = Some(sf_keybind);
                } else {
                    entry.0 = Some(sf_keybind);
                }
            }
        }
        // Then extract all hashmap values into a vec
        let mut return_vec = Vec::new();
        for (_k, v) in result.drain() {
            return_vec.push(v);
        }
        return_vec
    }

    fn get_sf_keybind_on_release(
        &mut self,
        mask_key_pair: (u32, u32),
    ) -> Vec<(Option<&mut StatefullKeybind>, Option<&mut StatefullKeybind>)> {
        let mut sf_keybinds_with_mod;
        // In case modifier get released before key:
        // We receive a release event with 'key' containing the released modifier keycode.
        // So we check if released key is a modifier
        // If it is, we get modifier mask and consider released all keybinds containing this modifier.
        if let Some(key_mask) =
            xkeysym_lookup::mask_from_keysym(mask_key_pair.1, &self.xwrap.modifier_mapping)
        {
            sf_keybinds_with_mod = self.get_sf_keybind_pair_list_from_key_mod(Some(key_mask), None);
        } else {
            sf_keybinds_with_mod =
                self.get_sf_keybind_pair_list_from_key_mod(None, Some(mask_key_pair.1));
        }
        let sf_keybind_vec: Vec<(Option<&mut StatefullKeybind>, Option<&mut StatefullKeybind>)> =
            sf_keybinds_with_mod
                .drain(..)
                // Only keep keybinds pair that have a on_release element
                .filter(|(_press_keybind, release_keybind)| release_keybind.is_some())
                // Only keep keybinds that are currently pressed
                .filter(|(_press_keybind, release_keybind)| {
                    if let Some(kb) = release_keybind {
                        kb.already_pressed
                    } else {
                        false
                    }
                })
                .collect();
        sf_keybind_vec
    }

    fn handle_mapping_notify(&mut self, event: &mut xlib::XMappingEvent) -> Error {
        if event.request == xlib::MappingModifier || event.request == xlib::MappingKeyboard {
            return self.xwrap.refresh_keyboard(event);
        }
        Ok(())
    }
}
