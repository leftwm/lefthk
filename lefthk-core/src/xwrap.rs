use crate::errors::{self, Error, LeftError};
use crate::worker::StatefullKeybind;
use crate::xkeysym_lookup;
use std::collections::HashMap;
use std::future::Future;
use std::os::raw::{c_int, c_ulong};
use std::pin::Pin;
use std::ptr;
use std::sync::Arc;
use tokio::sync::{Notify, oneshot};
use tokio::time::Duration;
use x11_dl::xlib;

pub struct XWrap {
    pub xlib: xlib::Xlib,
    pub display: *mut xlib::Display,
    pub root: xlib::Window,
    pub task_notify: Arc<Notify>,
    _task_guard: oneshot::Receiver<()>,
    pub modifier_mapping: HashMap<String, Vec<u32>>,
}

impl Default for XWrap {
    fn default() -> Self {
        Self::new()
    }
}

impl XWrap {
    /// # Panics
    ///
    /// Panics if unable to contact xorg.
    #[must_use]
    #[allow(clippy::items_after_statements)]
    pub fn new() -> Self {
        const extern "C" fn on_error_from_xlib(
            _: *mut xlib::Display,
            er: *mut xlib::XErrorEvent,
        ) -> c_int {
            let err = unsafe { *er };
            //ignore bad window errors
            if err.error_code == xlib::BadWindow {
                return 0;
            }
            1
        }
        const SERVER: mio::Token = mio::Token(0);
        let xlib = errors::exit_on_error!(xlib::Xlib::open());
        let display = unsafe { (xlib.XOpenDisplay)(ptr::null()) };
        assert!(!display.is_null(), "Null pointer in display");

        let fd = unsafe { (xlib.XConnectionNumber)(display) };
        let (guard, task_guard) = oneshot::channel();
        let notify = Arc::new(Notify::new());
        let task_notify = notify.clone();
        let mut poll = errors::exit_on_error!(mio::Poll::new());
        let mut events = mio::Events::with_capacity(1);
        errors::exit_on_error!(poll.registry().register(
            &mut mio::unix::SourceFd(&fd),
            SERVER,
            mio::Interest::READABLE,
        ));
        let timeout = Duration::from_millis(100);
        tokio::task::spawn_blocking(move || {
            loop {
                if guard.is_closed() {
                    return;
                }

                if let Err(err) = poll.poll(&mut events, Some(timeout)) {
                    tracing::warn!("Xlib socket poll failed with {:?}", err);
                    continue;
                }

                events
                    .iter()
                    .filter(|event| SERVER == event.token())
                    .for_each(|_| notify.notify_one());
            }
        });
        let root = unsafe { (xlib.XDefaultRootWindow)(display) };

        let modifier_mapping = HashMap::new();
        let xw = Self {
            xlib,
            display,
            root,
            task_notify,
            _task_guard: task_guard,
            modifier_mapping,
        };

        // Setup cached keymap/modifier information, otherwise MappingNotify might never be called
        // from:
        // https://stackoverflow.com/questions/35569562/how-to-catch-keyboard-layout-change-event-and-get-current-new-keyboard-layout-on
        let _ = xw.keysym_to_keycode(x11_dl::keysym::XK_F1);

        unsafe {
            (xw.xlib.XSetErrorHandler)(Some(on_error_from_xlib));
            (xw.xlib.XSync)(xw.display, xlib::False);
        };
        xw
    }

    /// Shutdown connections to the xserver.
    pub fn shutdown(&self) {
        unsafe {
            (self.xlib.XUngrabKey)(self.display, xlib::AnyKey, xlib::AnyModifier, self.root);
            (self.xlib.XCloseDisplay)(self.display);
        }
    }

    /// Set `DetectableAutoRepeat` to catch press/release buttons
    pub fn set_detectable_auto_repeat(&self) -> bool {
        let mut detectable_autorepeat: c_int = c_int::from(false);
        unsafe {
            (self.xlib.XkbSetDetectableAutoRepeat)(
                self.display,
                c_int::from(true),
                &raw mut detectable_autorepeat,
            );
        }
        if detectable_autorepeat != 0 {
            tracing::info!("Xserver activated detectable autorepeat correctly.");
            true
        } else {
            tracing::warn!(
                "Xserver could not activate detectable autorepeat. This feature will be disabled."
            );
            false
        }
    }

    /// Grabs a list of keybindings.
    pub fn grab_keys(&self, sf_keybinds: &[StatefullKeybind]) {
        // Cleanup key grabs.
        unsafe {
            (self.xlib.XUngrabKey)(self.display, xlib::AnyKey, xlib::AnyModifier, self.root);
        }

        // Grab all the key combos from the config file.
        for sf_kb in sf_keybinds {
            if let Some(keysym) = xkeysym_lookup::into_keysym(&sf_kb.keybind.key) {
                let modmask = xkeysym_lookup::into_modmask(&sf_kb.keybind.modifier);
                self.grab_key(self.root, keysym, modmask);
            }
        }
    }

    /// Grabs the keysym with the modifier for a window.
    pub fn grab_key(&self, root: xlib::Window, keysym: u32, modifiers: u32) {
        let code = unsafe { (self.xlib.XKeysymToKeycode)(self.display, c_ulong::from(keysym)) };
        // Grab the keys with and without numlock (Mod2).
        let mods: Vec<u32> = vec![
            modifiers,
            modifiers | xlib::Mod2Mask,
            modifiers | xlib::LockMask,
        ];
        for m in mods {
            unsafe {
                (self.xlib.XGrabKey)(
                    self.display,
                    i32::from(code),
                    m,
                    root,
                    1,
                    xlib::GrabModeAsync,
                    xlib::GrabModeAsync,
                );
            }
        }
    }

    /// Updates the keyboard mapping.
    /// # Errors
    ///
    /// Will error if updating the keyboard failed.
    pub fn refresh_keyboard(&mut self, evt: &mut xlib::XMappingEvent) -> Error {
        let status = unsafe { (self.xlib.XRefreshKeyboardMapping)(evt) };
        if status == 0 {
            Err(LeftError::XFailedStatus)
        } else {
            self.get_modifier_mapping();
            Ok(())
        }
    }

    /// Get modifier mapping
    pub fn get_modifier_mapping(&mut self) {
        let modifiers = [
            "Shift".to_string(),
            "NumLock".to_string(),
            "Control".to_string(),
            "Mod1".to_string(),
            "Mod2".to_string(),
            "Mod3".to_string(),
            "Mod4".to_string(),
            "Mod5".to_string(),
        ];
        unsafe {
            let modifier_mapping_pointer = (self.xlib.XGetModifierMapping)(self.display);
            for (modifier, i) in modifiers.iter().zip(0isize..) {
                // We use Vec here as max_keypermod is unknown
                let mut mapping = Vec::new();
                let max_keypermod = (*modifier_mapping_pointer).max_keypermod;
                for j in 0..max_keypermod {
                    let keycode = *(*modifier_mapping_pointer)
                        .modifiermap
                        .offset(i * (max_keypermod as isize) + (j as isize));
                    if let Ok(keysym) = self.keycode_to_keysym(keycode.into()) {
                        mapping.push(keysym);
                    } else {
                        tracing::warn!(
                            "Could not convert keycode {keycode} provided by XGetModifierMapping to keysym. Ignoring that key."
                        );
                    }
                }
                self.modifier_mapping.insert(modifier.clone(), mapping);
            }
        }
    }

    /// Converts a keycode to a keysym.
    ///
    /// # Errors
    /// - `LeftError::TryFromIntError`: if the `keycode` argument exceeds the u8 limit, it will not
    ///   be coercable from u32 to u8 and will thus return an error. Potentially the conversion of
    ///   the output of `XkbKeycodeToKeysym` coversion to u32 may overflow in this manner also.
    pub fn keycode_to_keysym(&self, keycode: u32) -> Result<xkeysym_lookup::XKeysym, LeftError> {
        // Not using XKeysymToKeycode because deprecated.
        let sym =
            unsafe { (self.xlib.XkbKeycodeToKeysym)(self.display, u8::try_from(keycode)?, 0, 0) };
        Ok(u32::try_from(sym)?)
    }

    /// Converts a keysym to a keycode.
    #[must_use]
    pub fn keysym_to_keycode(&self, keysym: xkeysym_lookup::XKeysym) -> u32 {
        let code = unsafe { (self.xlib.XKeysymToKeycode)(self.display, keysym.into()) };
        u32::from(code)
    }

    /// Returns the next `Xevent` of the xserver.
    #[must_use]
    pub fn get_next_event(&self) -> xlib::XEvent {
        unsafe {
            let mut event: xlib::XEvent = std::mem::zeroed();
            (self.xlib.XNextEvent)(self.display, &raw mut event);
            event
        }
    }

    /// Returns how many events are waiting.
    #[must_use]
    pub fn queue_len(&self) -> i32 {
        unsafe { (self.xlib.XPending)(self.display) }
    }

    pub fn flush(&self) {
        unsafe { (self.xlib.XFlush)(self.display) };
    }

    pub fn wait_readable(&mut self) -> Pin<Box<dyn Future<Output = ()>>> {
        let task_notify = self.task_notify.clone();
        Box::pin(async move {
            task_notify.notified().await;
        })
    }
}
