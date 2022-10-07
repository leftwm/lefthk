pub mod context;

use std::collections::HashMap;

use crate::child::Children;
use crate::config::command::CommandId;
use crate::config::{Keybind, command};
use crate::errors::{self, Error, LeftError};
use crate::ipc::Pipe;
use crate::xkeysym_lookup;
use crate::xwrap::XWrap;
use command::Command;
use x11_dl::xlib;
use xdg::BaseDirectories;

use self::context::Context;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Status {
    Reload,
    Kill,
    Continue,
}

pub struct Worker {
    pub keybinds: Vec<Keybind>,
    pub base_directory: BaseDirectories,
    pub xwrap: XWrap,
    pub children: Children,
    pub status: Status,

    pub ctxs: HashMap<CommandId, Box<dyn Context>>,

    pub reload_ctx: context::Reload,
    pub kill_ctx: context::Kill,
    pub chord_ctx: context::Chord,
}

impl Worker {
    pub fn new(keybinds: Vec<Keybind>, base_directory: BaseDirectories) -> Self {
        let ctxs = vec![
        ];
        Self {
            status: Status::Continue,
            keybinds,
            base_directory,
            xwrap: XWrap::new(),
            children: Default::default(),
            reload_ctx: context::Reload::new(),
            kill_ctx: context::Kill::new(),
            chord_ctx: context::Chord::new(),
        }
    }

    pub async fn event_loop(mut self) -> Status {
        self.xwrap.grab_keys(&self.keybinds);
        let pipe_name = Pipe::pipe_name();
        let pipe_file = errors::exit_on_error!(self.base_directory.place_runtime_file(pipe_name));
        let mut pipe = errors::exit_on_error!(Pipe::new(pipe_file).await);

        while self.status == Status::Continue {
            self.xwrap.flush();

            for context in self.ctxs {
                context.evaluate(&mut self);
            }

            // if self.kill_ctx.requested || self.reload_ctx.requested {
            //     return self.kill_ctx.requested;
            // }
            //
            // if self.chord_elapsed {
            //     self.xwrap.grab_keys(&self.keybinds);
            //     self.chord_keybinds = None;
            //     self.chord_elapsed = false;
            // }

            tokio::select! {
                _ = self.children.wait_readable() => {
                    self.children.reap();
                }
                _ = self.xwrap.wait_readable() => {
                    let event_in_queue = self.xwrap.queue_len();
                    for _ in 0..event_in_queue {
                        let xlib_event = self.xwrap.get_next_event();
                        self.handle_event(&xlib_event);
                    }
                }
                Some(command) = pipe.read_command() => {
                    command.execute(&mut self);
                }
            }
        }

        self.status
    }

    fn handle_event(&mut self, xlib_event: &xlib::XEvent) {
        let error = match xlib_event.get_type() {
            xlib::KeyPress => self.key_press(&xlib::XKeyEvent::from(xlib_event)),
            xlib::MappingNotify => self.mapping_notify(&mut xlib::XMappingEvent::from(xlib_event)),
            _ => return,
        };
        errors::log_on_error!(error);
    }

    fn key_press(&mut self, event: &xlib::XKeyEvent) -> Error {
        let key = self.xwrap.keycode_to_keysym(event.keycode);
        let mask = xkeysym_lookup::clean_mask(event.state);
        if let Some(keybind) = self.get_keybind((mask, key)) {
            if let Ok(command) = command::denormalize(keybind.command) {
                return command.execute(&mut self);
            }
        } else {
            return Err(LeftError::CommandNotFound);
        }
        Ok(())
    }

    fn get_keybind(&self, mask_key_pair: (u32, u32)) -> Option<Keybind> {
        let keybinds = if let Some(keybinds) = &self.chord_ctx.keybinds {
            keybinds
        } else {
            &self.keybinds
        };
        keybinds
            .iter()
            .find(|keybind| {
                if let Some(key) = xkeysym_lookup::into_keysym(&keybind.key) {
                    let mask = xkeysym_lookup::into_modmask(&keybind.modifier);
                    return mask_key_pair == (mask, key);
                }
                false
            })
            .cloned()
    }

    fn mapping_notify(&self, event: &mut xlib::XMappingEvent) -> Error {
        if event.request == xlib::MappingModifier || event.request == xlib::MappingKeyboard {
            return self.xwrap.refresh_keyboard(event);
        }
        Ok(())
    }
}
