use crate::config::Keybind;
use crate::errors::{self, Error, LeftError};
use crate::xkeysym_lookup;
use std::future::Future;
use std::os::raw::{c_int, c_ulong};
use std::pin::Pin;
use std::ptr;
use std::sync::Arc;
use tokio::sync::{oneshot, Notify};
use tokio::time::Duration;
use x11_dl::xlib;

pub struct XWrap {
    pub xlib: xlib::Xlib,
    pub display: *mut xlib::Display,
    pub root: xlib::Window,
    pub task_notify: Arc<Notify>,
    _task_guard: oneshot::Receiver<()>,
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
    pub fn new() -> Self {
        const SERVER: mio::Token = mio::Token(0);
        let xlib = errors::exit_on_error!(xlib::Xlib::open());
        let display = unsafe { (xlib.XOpenDisplay)(ptr::null()) };
        assert!(!display.is_null(), "Null pointer in display");

        let fd = unsafe { (xlib.XConnectionNumber)(display) };
        let (guard, _task_guard) = oneshot::channel::<()>();
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
        tokio::task::spawn_blocking(move || loop {
            if guard.is_closed() {
                return;
            }

            if let Err(err) = poll.poll(&mut events, Some(timeout)) {
                log::warn!("Xlib socket poll failed with {:?}", err);
                continue;
            }

            events
                .iter()
                .filter(|event| SERVER == event.token())
                .for_each(|_| notify.notify_one());
        });
        let root = unsafe { (xlib.XDefaultRootWindow)(display) };

        let xw = Self {
            xlib,
            display,
            root,
            task_notify,
            _task_guard,
        };

        // Setup cached keymap/modifier information, otherwise MappingNotify might never be called
        // from:
        // https://stackoverflow.com/questions/35569562/how-to-catch-keyboard-layout-change-event-and-get-current-new-keyboard-layout-on
        xw.keysym_to_keycode(x11_dl::keysym::XK_F1);

        // This is allowed for now as const extern fns
        // are not yet stable (1.56.0, 16 Sept 2021)
        // see issue #64926 <https://github.com/rust-lang/rust/issues/64926> for more information
        #[allow(clippy::missing_const_for_fn)]
        extern "C" fn on_error_from_xlib(
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

    /// Grabs a list of keybindings.
    pub fn grab_keys(&self, keybinds: &[Keybind]) {
        // Cleanup key grabs.
        unsafe {
            (self.xlib.XUngrabKey)(self.display, xlib::AnyKey, xlib::AnyModifier, self.root);
        }

        // Grab all the key combos from the config file.
        for kb in keybinds {
            if let Some(keysym) = xkeysym_lookup::into_keysym(&kb.key) {
                let modmask = xkeysym_lookup::into_modmask(&kb.modifier);
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
    pub fn refresh_keyboard(&self, evt: &mut xlib::XMappingEvent) -> Error {
        let status = unsafe { (self.xlib.XRefreshKeyboardMapping)(evt) };
        if status == 0 {
            Err(LeftError::XFailedStatus)
        } else {
            Ok(())
        }
    }

    /// Converts a keycode to a keysym.
    #[must_use]
    pub fn keycode_to_keysym(&self, keycode: u32) -> xkeysym_lookup::XKeysym {
        // Not using XKeysymToKeycode because deprecated.
        let sym = unsafe { (self.xlib.XkbKeycodeToKeysym)(self.display, keycode as u8, 0, 0) };
        sym as u32
    }

    /// Converts a keysym to a keycode.
    pub fn keysym_to_keycode(&self, keysym: xkeysym_lookup::XKeysym) -> u32 {
        let code = unsafe { (self.xlib.XKeysymToKeycode)(self.display, keysym.into()) };
        u32::from(code)
    }

    /// Returns the next `Xevent` of the xserver.
    #[must_use]
    pub fn get_next_event(&self) -> xlib::XEvent {
        unsafe {
            let mut event: xlib::XEvent = std::mem::zeroed();
            (self.xlib.XNextEvent)(self.display, &mut event);
            event
        }
    }

    /// Returns how many events are waiting.
    #[must_use]
    pub fn queue_len(&self) -> i32 {
        unsafe { (self.xlib.XPending)(self.display) }
    }

    /// Wait until readable.
    pub fn wait_readable(&mut self) -> Pin<Box<dyn Future<Output = ()>>> {
        let task_notify = self.task_notify.clone();
        Box::pin(async move {
            task_notify.notified().await;
        })
    }
}
