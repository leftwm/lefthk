use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum Key {
    Key(String),
    Keys(Vec<String>),
}

