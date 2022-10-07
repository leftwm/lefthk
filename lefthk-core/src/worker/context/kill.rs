#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Kill {
    pub requested: bool,
}

impl Kill {
    pub fn new() -> Self {
        Self {
            requested: false,
        }
    }
}
