use std::collections::HashMap;
use std::iter::{Extend, FromIterator};
use std::process::Child;
use std::sync::{atomic::AtomicBool, Arc};

/// A struct managing children processes.
///
/// The `reap` method could be called at any place the user wants to.
/// `register_child_hook` provides a hook that sets a flag. User may use the
/// flag to do a epoch-based reaping.
#[derive(Debug, Default)]
pub struct Children {
    inner: HashMap<u32, Child>,
}

impl Children {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }
    /// Insert a `Child` in the `Children`.
    /// If this `Children` did not have this value present, true is returned.
    /// If this `Children` did have this value present, false is returned.
    pub fn insert(&mut self, child: Child) -> bool {
        // Not possible to have duplication!
        self.inner.insert(child.id(), child).is_none()
    }
    /// Merge another `Children` into this `Children`.
    pub fn merge(&mut self, reaper: Self) {
        self.inner.extend(reaper.inner.into_iter());
    }
    /// Try reaping all the children processes managed by this struct.
    pub fn reap(&mut self) {
        println!("Reaping");
        // The `try_wait` needs `child` to be `mut`, but only `HashMap::retain`
        // allows modifying the value. Here `id` is not needed.
        self.inner
            .retain(|_, child| child.try_wait().map_or(true, |ret| ret.is_none()));
    }
}

impl FromIterator<Child> for Children {
    fn from_iter<T: IntoIterator<Item = Child>>(iter: T) -> Self {
        Self {
            inner: iter
                .into_iter()
                .map(|child| (child.id(), child))
                .collect::<HashMap<_, _>>(),
        }
    }
}

impl Extend<Child> for Children {
    fn extend<T: IntoIterator<Item = Child>>(&mut self, iter: T) {
        self.inner
            .extend(iter.into_iter().map(|child| (child.id(), child)));
    }
}

/// Register the `SIGCHLD` signal handler. Once the signal is received,
/// the flag will be set true. User needs to manually clear the flag.
pub fn register_child_hook(flag: Arc<AtomicBool>) {
    let _ = signal_hook::flag::register(signal_hook::consts::signal::SIGCHLD, flag)
        .map_err(|err| log::error!("Cannot register SIGCHLD signal handler: {:?}", err));
}
