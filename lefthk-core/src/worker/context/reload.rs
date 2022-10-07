#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reload {
    pub requested: bool
}

impl Reload {
    pub fn new() -> Self {
        Self {
            requested: false,
        }
    }
}
