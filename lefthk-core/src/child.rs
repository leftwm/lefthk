use std::iter::Extend;
use std::pin::Pin;
use std::process::Child;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, future::Future};

use signal_hook::consts::signal;
use signal_hook::iterator::Signals;
use tokio::sync::{oneshot, Notify};

/// A struct managing children processes.
///
/// The `reap` method could be called at any place the user wants to.
/// `register_child_hook` provides a hook that sets a flag. User may use the
/// flag to do a epoch-based reaping.
#[derive(Debug)]
pub struct Children {
    inner: HashMap<u32, Child>,
    pub task_notify: Arc<Notify>,
    _task_guard: oneshot::Receiver<()>,
}

impl Default for Children {
    fn default() -> Self {
        Self::new()
    }
}

impl Children {
    pub fn new() -> Self {
        let (guard, _task_guard) = oneshot::channel();
        let task_notify = Arc::new(Notify::new());
        let notify = task_notify.clone();
        let mut signals = Signals::new(&[signal::SIGCHLD]).expect("Couldn't setup signals.");
        tokio::task::spawn_blocking(move || loop {
            if guard.is_closed() {
                return;
            }
            for _ in signals.pending() {
                notify.notify_one();
            }
            std::thread::sleep(Duration::from_millis(100));
        });

        Self {
            task_notify,
            _task_guard,
            inner: HashMap::default(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    /// Add another child-process.
    /// ## Return value
    /// `false` if it's already registered, otherwise `true`
    pub fn insert(&mut self, child: Child) -> bool {
        // Not possible to have duplication!
        self.inner.insert(child.id(), child).is_none()
    }

    /// Merge another `Children` into this `Children`.
    pub fn merge(&mut self, reaper: Self) {
        self.inner.extend(reaper.inner.into_iter());
    }

    /// Remove all children which finished
    pub fn reap(&mut self) {
        self.inner
            .retain(|_, child| child.try_wait().map_or(true, |ret| ret.is_none()));
    }

    pub fn wait_readable(&mut self) -> Pin<Box<dyn Future<Output = ()>>> {
        let task_notify = self.task_notify.clone();
        Box::pin(async move {
            task_notify.notified().await;
        })
    }
}

impl Extend<Child> for Children {
    fn extend<T: IntoIterator<Item = Child>>(&mut self, iter: T) {
        self.inner
            .extend(iter.into_iter().map(|child| (child.id(), child)));
    }
}
