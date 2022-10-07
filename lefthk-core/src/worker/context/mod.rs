mod kill;
mod chord;
mod reload;

pub use kill::Kill;
pub use chord::Chord;
pub use reload::Reload;

use super::Worker;

pub trait Context {
    fn evaluate(&self, worker: &mut Worker);
}
