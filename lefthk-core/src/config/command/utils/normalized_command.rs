use std::fmt::Display;

use serde::{Deserialize, Serialize};

type Content = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NormalizedCommand(pub Content);

impl TryFrom<String> for NormalizedCommand {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self(value))
    }
}

impl Display for NormalizedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
